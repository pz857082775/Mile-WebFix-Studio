import type { Finding, PageId, Summary } from './types';

export const PAGE_META: Record<PageId, [string, string]> = {
  scan: ['一键体检', '单页安全与质量体检'],
  crawl: ['全站爬行', '自动发现同域路由并聚合体检'],
  inspect: ['人工巡检', '内嵌浏览器实时抓取'],
  fix: ['修复工作台', '七字段 → 四件套修复方案'],
  kb: ['BUG 知识库', '规则与挖掘模板'],
  ai: ['AI 设置', '零成本 / AI 增强双模式'],
  ext: ['扩展中心', '智能体 · SKILL · MCP · Bot'],
  orch: ['协同编排', '串行 / 并行 / 条件流程'],
};

export const PAGE_IDS = Object.keys(PAGE_META) as PageId[];

export const SEV: Record<string, string> = { critical: '严重', high: '高危', medium: '中危', low: '低危' };
export const CAT: Record<string, string> = {
  ui: '界面', logic: '逻辑', function: '功能', data: '数据', algorithm: '算法',
  security: '安全', performance: '性能', accessibility: '无障碍',
};
export const CAT_COLOR: Record<string, string> = {
  ui: 'pink', logic: 'purple', function: 'blue', data: 'cyan',
  algorithm: 'geek', security: 'red', performance: 'orange', accessibility: 'green',
};
export const EV_LABEL: Record<string, string> = {
  console: '控制台', exception: 'JS 异常', network: '网络', resource: '资源', dom: 'DOM', page: '页面', scan: '体检',
};

export function summaryOf(findings: Finding[]): Summary {
  const bySeverity: Record<string, number> = {};
  const byCategory: Record<string, number> = {};
  for (const f of findings) {
    bySeverity[f.severity] = (bySeverity[f.severity] || 0) + 1;
    byCategory[f.category] = (byCategory[f.category] || 0) + 1;
  }
  return { total: findings.length, bySeverity, byCategory };
}

export function findingKey(f: Finding, url: string): string {
  return (f.ruleId || f.title) + '|' + url;
}

export function fmtTime(ts: number): string {
  return new Date(ts).toTimeString().slice(0, 8);
}