/** Single analysis result (one file or stdin). */
export interface AnalysisResult {
  filePath: string;
  score: { value: number; grade: string };
  breakdown: Record<string, number>;
  issues: Issue[];
  stats: { totalTests: number; totalAssertions: number; [k: string]: unknown };
  framework: string;
  testType: string;
  sourceFile?: string;
  testScores?: unknown[];
  transparentBreakdown?: unknown;
}

export interface Issue {
  rule: string;
  severity: string;
  message: string;
  location: { line: number; column: number };
  suggestion?: string;
  fix?: { startLine: number; startColumn: number; endLine: number; endColumn: number; replacement: string };
}

export interface AnalyzeOptions {
  config?: string;
  threshold?: number;
}

/**
 * Run rigor and return parsed JSON result.
 * @param input - File path or { stdin: sourceCode, filename?: string }
 */
export function analyze(
  input: string | { stdin: string; filename?: string },
  options?: AnalyzeOptions
): Promise<AnalysisResult | AnalysisResult[]>;

/**
 * Analyze test source from a string (stdin mode).
 */
export function analyzeSource(
  source: string,
  options?: AnalyzeOptions & { filename?: string }
): Promise<AnalysisResult>;
