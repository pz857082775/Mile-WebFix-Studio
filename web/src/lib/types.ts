export type PageId = 'scan' | 'crawl' | 'inspect' | 'fix' | 'kb' | 'ai' | 'ext' | 'orch';

export interface Finding {
  ruleId?: string | null;
  title: string;
  url?: string;
  severity: string;
  category: string;
  evidence?: string;
  steps?: string;
  rootCause?: string;
  fix?: { frontend: string; backend: string };
  verify?: string[];
  guard?: string;
  source?: string;
  id?: string;
  urls?: string[];
}

export interface Rule {
  id: string;
  manual: boolean;
  category: string;
  severity: string;
  title: string;
  rootCause: string;
  test: string;
  fix: { frontend: string; backend: string };
  verify: string[];
  guard: string;
  diff?: string;
}

export interface Summary {
  total: number;
  bySeverity: Record<string, number>;
  byCategory: Record<string, number>;
}

export interface ScanResult {
  target: string;
  finalUrl: string;
  status: number;
  scannedAt: string;
  durationMs: number;
  findings: Finding[];
  links: string[];
  headers: Record<string, string>;
  html: string;
  summary: Summary;
}

export interface CrawlJob {
  id: string;
  seed: string;
  status: string;
  error?: string;
  scanned: number;
  maxPages: number;
  pages: { url: string; status: number; findings: number }[];
  findings: Finding[];
  summary: Summary;
  durationMs: number;
}

export interface CaptureEvent {
  type: string;
  level?: string;
  message?: string;
  url?: string;
  status?: number;
  tag?: string;
  source?: string;
  stack?: string;
  issues?: { ruleId: string; evidence: string }[];
  title?: string;
  pageUrl?: string;
  realUrl?: string;
  ts: number;
}

export interface InspectSession {
  sid: string;
  token: string;
  proxyUrl: string;
  cookieName: string;
  cookiePath: string;
  origin: string;
  findings: Map<string, Finding>;
  events: CaptureEvent[];
}

export interface Extension {
  id: string;
  type: string;
  transport: string;
  name: string;
  version: string;
  description: string;
  enabled: boolean;
  builtin: boolean;
  endpoint: string;
  command: string;
  args: string[];
  module: string;
  config: Record<string, unknown>;
  permissions: string[];
  createdAt: number;
  updatedAt?: number;
}

export interface PipelineStep {
  key: string;
  ext: string;
  input: Record<string, unknown>;
  onError: string;
  retry: number;
  timeoutMs: number;
  when?: { path: string; op: string; value?: unknown };
}

export interface Pipeline {
  id: string;
  name: string;
  description: string;
  mode: string;
  steps: PipelineStep[];
  builtin: boolean;
  createdAt?: number;
  updatedAt?: number;
}

export interface PipelineRun {
  id: string;
  pipelineId: string;
  name: string;
  mode: string;
  status: string;
  startedAt: number;
  finishedAt: number | null;
  steps: { key: string; ext: string; status: string; ms: number; findings: number; error?: string; reason?: string }[];
  summary: Summary | null;
  error: string | null;
  stats: { total: number; critical: number; high: number; medium: number; low: number };
  findings: Finding[];
  outputs: Record<string, unknown>;
}

export interface AiConfig {
  enabled: boolean;
  provider: string;
  baseUrl: string;
  model: string;
  hasKey: boolean;
  keyTail: string;
}

export interface AiProvider {
  id: string;
  name: string;
  baseUrl: string;
  models: string[];
}

export interface FixResult {
  fields: Record<string, string>;
  deliverables: {
    minimalFix: { frontend: string; backend: string };
    verifySteps: string[];
    guardCode: string;
    gitDiff: string;
  };
  prompt: string;
  fullPrompt: string;
}