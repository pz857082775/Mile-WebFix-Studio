import { useEffect, useState } from 'react';
import { Modal } from './Modal';

export function About({ onClose }: { onClose: () => void }) {
  const [meta, setMeta] = useState<{ version?: string; configDir?: string }>({});
  useEffect(() => {
    fetch('/api/meta').then((r) => r.json()).then(setMeta).catch(() => {});
  }, []);
  return (
    <Modal title="关于 WebFix Studio" onClose={onClose} small footer={<button className="btn" onClick={onClose}>确定</button>}>
      <div className="flex items-center gap-3">
        <img src="/logo.png" alt="WebFix Studio" className="w-12 h-12 rounded-[12px] shadow-chip" />
        <div>
          <div className="text-[15px] font-bold">WebFix Studio</div>
          <div className="text-xs text-ink-3 mt-0.5">一站式网页 BUG / 漏洞巡检与修复工作台</div>
        </div>
        <span className="badge cat ml-auto">v{meta.version || '1.1.0'}</span>
      </div>
      <p className="text-xs text-ink-2 leading-relaxed mt-4">
        自动体检、全站爬行、人工巡检 + AI 自动操作识别；261 条安全知识规则，
        七字段 → 四件套修复方案可直接提交。网页版与桌面版界面功能完全一致。
      </p>
      <div className="mt-4 rounded-lg border border-line bg-side px-4 py-3 text-xs leading-relaxed text-ink-2 space-y-1">
        <div className="flex justify-between"><span className="text-ink-3">版权</span><span>© 米乐</span></div>
        <div className="flex justify-between"><span className="text-ink-3">微信</span><span>mileai8</span></div>
        <div className="flex justify-between"><span className="text-ink-3">邮箱</span><span>857082775@qq.com</span></div>
        <div className="flex justify-between"><span className="text-ink-3">平台</span><span>Windows / Linux / macOS / Web</span></div>
        <div className="flex justify-between gap-3"><span className="text-ink-3 shrink-0">配置目录</span><span className="text-right break-all">{meta.configDir || '…'}</span></div>
      </div>
      <p className="text-[11px] text-ink-3 mt-3">仅可扫描自有或已获授权的网站 · AI 密钥仅存本机 · 配置保存于系统用户目录，重装 / 升级不丢失</p>
    </Modal>
  );
}