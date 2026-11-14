// 从旧版 JS 引擎精确导出知识库 → assets/knowledge.json（Rust include_str! 内嵌，保证逐条一致）
import { writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { RULES, MANUAL, CAPTURE_RULES } from '../../webfix-studio/lib/rules.js';
import { KNOWLEDGE } from '../../webfix-studio/lib/knowledge.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const out = {
  version: 1,
  generatedFrom: 'webfix-studio/lib/rules.js + knowledge.js',
  rules: RULES,
  manual: MANUAL,
  captureRules: CAPTURE_RULES,
  knowledge: KNOWLEDGE,
};
const file = path.join(__dirname, '..', 'assets', 'knowledge.json');
mkdirSync(path.dirname(file), { recursive: true });
writeFileSync(file, JSON.stringify(out, null, 1));
const count = (o) => Object.keys(o).length;
console.log(`rules=${count(RULES)} manual=${count(MANUAL)} capture=${count(CAPTURE_RULES)} knowledge=${count(KNOWLEDGE)} total=${count(RULES) + count(MANUAL) + count(CAPTURE_RULES) + count(KNOWLEDGE)}`);
console.log('written:', file);