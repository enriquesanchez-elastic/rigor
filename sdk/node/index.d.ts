/** Grade letter (A–F) */
export type Grade = "A" | "B" | "C" | "D" | "F";

/** Severity level for an issue */
export type Severity = "error" | "warning" | "info";

/** Score with numeric value and letter grade */
export interface Score {
  value: number;
  grade: Grade;
}

/** Category breakdown in the scoring model */
export interface ScoreBreakdown {
  assertionQuality: number;
  errorCoverage: number;
  boundaryConditions: number;
  testIsolation: number;
  inputVariety: number;
  aiSmells: number;
}

/** Single category entry in the transparent breakdown */
export interface CategoryBreakdownEntry {
  categoryName: string;
  rawScore: number;
  maxRaw: number;
  weightPct: number;
  weightedContribution: number;
}

/** Full transparent breakdown showing how the score was calculated */
export interface TransparentBreakdown {
  categories: CategoryBreakdownEntry[];
  totalBeforePenalties: number;
  penaltyTotal: number;
  penaltyFromErrors: number;
  penaltyFromWarnings: number;
  penaltyFromInfo: number;
  finalScore: number;
}

/** Auto-fix metadata for an issue */
export interface Fix {
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
  replacement: string;
}

/** A single issue found during analysis */
export interface Issue {
  rule: string;
  severity: Severity;
  message: string;
  location: {
    line: number;
    column: number;
    endLine?: number;
    endColumn?: number;
  };
  suggestion?: string;
  fix?: Fix;
}

/** Per-test score within a file */
export interface TestScore {
  name: string;
  line: number;
  endLine?: number;
  score: number;
  grade: Grade;
  issues: Issue[];
}

/** Function coverage metrics (when source file is available) */
export interface FunctionCoverage {
  totalExports: number;
  testedExports: number;
  untestedExports: string[];
  coveragePercent: number;
}

/** Test statistics for an analyzed file */
export interface TestStats {
  totalTests: number;
  skippedTests: number;
  totalAssertions: number;
  describeBlocks: number;
  asyncTests: number;
  functionCoverage?: FunctionCoverage;
}

/** Single analysis result (one file or stdin) */
export interface AnalysisResult {
  filePath: string;
  score: Score;
  breakdown: ScoreBreakdown;
  transparentBreakdown?: TransparentBreakdown;
  testScores?: TestScore[];
  issues: Issue[];
  stats: TestStats;
  framework: string;
  testType: string;
  sourceFile?: string;
}

/** Options for the analyze function */
export interface AnalyzeOptions {
  /** Path to .rigorrc.json config file */
  config?: string;
  /** Minimum score threshold (exit 1 if below) */
  threshold?: number;
}

/**
 * Run rigor on a file path or stdin input and return parsed JSON result.
 *
 * @param input - File path, or `{ stdin: sourceCode, filename?: string }` for in-memory analysis
 * @param options - Optional configuration
 * @returns A single result for stdin/single file, or array for directory
 */
export function analyze(
  input: string | { stdin: string; filename?: string },
  options?: AnalyzeOptions
): Promise<AnalysisResult | AnalysisResult[]>;

/**
 * Analyze test source from a string (stdin mode).
 *
 * @param source - TypeScript test source code
 * @param options - Optional configuration, including virtual filename
 * @returns Analysis result for the provided source
 */
export function analyzeSource(
  source: string,
  options?: AnalyzeOptions & { filename?: string }
): Promise<AnalysisResult>;
