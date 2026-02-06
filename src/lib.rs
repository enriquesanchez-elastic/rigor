//! Rigor: Test Quality Analyzer for TypeScript
//!
//! This library provides static analysis of TypeScript test files to evaluate
//! test quality and provide actionable suggestions for improvement.

pub mod analyzer;
pub mod cache;
pub mod config;
pub mod coverage;
pub mod detector;
pub mod fixer;
pub mod history;
pub mod mcp;
pub mod mutation;
pub mod parser;
pub mod reporter;
pub mod suggestions;
pub mod watcher;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The main result of analyzing a test file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    /// Path to the analyzed test file
    pub file_path: PathBuf,
    /// Overall quality score (0-100)
    pub score: Score,
    /// Breakdown of scores by category
    pub breakdown: ScoreBreakdown,
    /// Full transparent breakdown (weights, penalties, traceable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparent_breakdown: Option<TransparentBreakdown>,
    /// Per-test scores (when available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_scores: Option<Vec<TestScore>>,
    /// List of issues found
    pub issues: Vec<Issue>,
    /// Statistics about the test file
    pub stats: TestStats,
    /// Detected test framework
    pub framework: TestFramework,
    /// Detected test type (unit, e2e, component, integration)
    #[serde(default)]
    pub test_type: TestType,
    /// Path to the corresponding source file (if found)
    pub source_file: Option<PathBuf>,
}

/// Quality score with grade
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    /// Numeric score (0-100)
    pub value: u8,
    /// Letter grade (A-F)
    pub grade: Grade,
}

impl Score {
    pub fn new(value: u8) -> Self {
        let grade = Grade::from_score(value);
        Self { value, grade }
    }
}

/// Score breakdown by category (each 0-25 points)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreBreakdown {
    /// Assertion quality score (0-25)
    pub assertion_quality: u8,
    /// Error coverage score (0-25)
    pub error_coverage: u8,
    /// Boundary condition coverage score (0-25)
    pub boundary_conditions: u8,
    /// Test isolation score (0-25)
    pub test_isolation: u8,
    /// Input variety score (0-25)
    pub input_variety: u8,
    /// AI-specific smell score (0-25)
    #[serde(default)]
    pub ai_smells: u8,
}

impl ScoreBreakdown {
    pub fn total(&self) -> u8 {
        // Six categories 0-25 each; normalize to 0-100 (sum * 100 / 150)
        let sum = self.assertion_quality as u16
            + self.error_coverage as u16
            + self.boundary_conditions as u16
            + self.test_isolation as u16
            + self.input_variety as u16
            + self.ai_smells as u16;
        ((sum * 100) / 150).min(100) as u8
    }
}

/// Per-category entry for transparent score breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryBreakdownEntry {
    /// Category name (e.g. "Assertion Quality")
    pub category_name: String,
    /// Raw score for this category (0-25)
    pub raw_score: u8,
    /// Maximum possible raw score (25)
    pub max_raw: u8,
    /// Weight percentage for this category (0-100)
    pub weight_pct: u8,
    /// Weighted contribution to total (before penalties)
    pub weighted_contribution: u8,
}

/// Full transparent breakdown: category scores, weights, and penalties
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparentBreakdown {
    /// Per-category scores and weights
    pub categories: Vec<CategoryBreakdownEntry>,
    /// Score before applying issue penalties
    pub total_before_penalties: u8,
    /// Total penalty deducted (from issues by severity)
    pub penalty_total: i32,
    /// Penalty from error-severity issues
    pub penalty_from_errors: i32,
    /// Penalty from warning-severity issues
    pub penalty_from_warnings: i32,
    /// Penalty from info-severity issues
    pub penalty_from_info: i32,
    /// Final score after penalties (0-100)
    pub final_score: u8,
}

/// Letter grade
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grade {
    A,
    B,
    C,
    D,
    F,
}

impl Grade {
    pub fn from_score(score: u8) -> Self {
        match score {
            90..=100 => Grade::A,
            80..=89 => Grade::B,
            70..=79 => Grade::C,
            60..=69 => Grade::D,
            _ => Grade::F,
        }
    }
}

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Grade::A => write!(f, "A"),
            Grade::B => write!(f, "B"),
            Grade::C => write!(f, "C"),
            Grade::D => write!(f, "D"),
            Grade::F => write!(f, "F"),
        }
    }
}

/// An issue found during analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Rule that found this issue
    pub rule: Rule,
    /// Severity of the issue
    pub severity: Severity,
    /// Human-readable message
    pub message: String,
    /// Location in the file
    pub location: Location,
    /// Suggested fix (if available)
    pub suggestion: Option<String>,
    /// Auto-fix: replacement text and range (when applicable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

/// A single auto-fix edit: replace the range with the replacement text
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    /// Start line (1-indexed)
    pub start_line: usize,
    /// Start column (1-indexed)
    pub start_column: usize,
    /// End line (1-indexed)
    pub end_line: usize,
    /// End column (1-indexed)
    pub end_column: usize,
    /// Replacement text
    pub replacement: String,
}

/// Severity levels for issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Analysis rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rule {
    WeakAssertion,
    MissingErrorTest,
    MissingBoundaryTest,
    SharedState,
    HardcodedValues,
    NoAssertions,
    SkippedTest,
    EmptyTest,
    DuplicateTest,
    LimitedInputVariety,
    DebugCode,
    FocusedTest,
    FlakyPattern,
    MockAbuse,
    SnapshotOveruse,
    VagueTestName,
    MissingAwait,
    RtlPreferScreen,
    RtlPreferSemantic,
    RtlPreferUserEvent,
    /// Assertion might let mutants survive (e.g. toBeGreaterThan(0) vs toBe(3))
    MutationResistant,
    /// Boundary test doesn't assert exact boundary value
    BoundarySpecificity,
    /// Test doesn't verify state changes, only return value
    StateVerification,
    /// Test name suggests a specific outcome but no assertion verifies it (relevance)
    AssertionIntentMismatch,
    /// Test only asserts on constants or trivial values (not meaningful)
    TrivialAssertion,
    /// Return paths in source not covered by tests
    ReturnPathCoverage,
    /// Test only verifies partial behavior (not full return shape)
    BehavioralCompleteness,
    /// Function has side effects but test doesn't verify them
    SideEffectNotVerified,
    // Phase 2.2 critical rules
    /// Test is too complex (high cyclomatic complexity or too many assertions)
    TestComplexity,
    /// Test is tightly coupled to implementation details
    ImplementationCoupling,
    /// Test is vacuous (e.g. always passes, no real verification)
    VacuousTest,
    /// Mock is used but not fully verified (e.g. toHaveBeenCalledWith)
    IncompleteMockVerification,
    /// Async error path not properly tested (rejects, catch)
    AsyncErrorMishandling,
    /// Redundant test (duplicates another test's coverage)
    RedundantTest,
    /// Unreachable code in test (after return/throw)
    UnreachableTestCode,
    /// Excessive setup (beforeEach/beforeAll doing too much)
    ExcessiveSetup,
    /// Overuse of type assertions (as Type) instead of real checks
    TypeAssertionAbuse,
    /// Missing cleanup (afterEach, reset mocks)
    MissingCleanup,
    // Phase 2.3 AI-specific smells
    /// Tautological assertion (e.g. expect(x).toBe(x))
    TautologicalAssertion,
    /// Over-mocking (too many mocks, testing implementation)
    OverMocking,
    /// Shallow variety (narrow input range)
    ShallowVariety,
    /// Happy-path-only (no error/edge tests)
    HappyPathOnly,
    /// Parrot assertion (repeats spec wording without real check)
    ParrotAssertion,
    /// Boilerplate padding (generic setup, low signal)
    BoilerplatePadding,
}

/// Scoring category name for transparent breakdown and verbose output.
/// Returns the category name if this rule affects a category score; None if it only affects penalty.
pub fn rule_scoring_category(rule: &Rule) -> Option<&'static str> {
    use Rule::*;
    match rule {
        WeakAssertion
        | NoAssertions
        | TrivialAssertion
        | AssertionIntentMismatch
        | MutationResistant
        | BoundarySpecificity
        | StateVerification
        | BehavioralCompleteness
        | SideEffectNotVerified => Some("Assertion Quality"),
        MissingErrorTest | ReturnPathCoverage => Some("Error Coverage"),
        MissingBoundaryTest => Some("Boundary Conditions"),
        SharedState => Some("Test Isolation"),
        HardcodedValues | LimitedInputVariety | DuplicateTest => Some("Input Variety"),
        // Phase 2.2 critical rules
        TestComplexity | VacuousTest | RedundantTest | UnreachableTestCode | ExcessiveSetup
        | TypeAssertionAbuse | MissingCleanup => Some("Assertion Quality"),
        ImplementationCoupling => Some("Test Isolation"),
        IncompleteMockVerification | AsyncErrorMishandling => Some("Error Coverage"),
        // Phase 2.3 AI smells (dedicated category)
        TautologicalAssertion
        | OverMocking
        | ShallowVariety
        | HappyPathOnly
        | ParrotAssertion
        | BoilerplatePadding => Some("AI Smells"),
        // These only affect penalty, not category score
        DebugCode | FocusedTest | SkippedTest | EmptyTest | FlakyPattern | MockAbuse
        | SnapshotOveruse | VagueTestName | MissingAwait | RtlPreferScreen | RtlPreferSemantic
        | RtlPreferUserEvent => None,
    }
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rule::WeakAssertion => write!(f, "weak-assertion"),
            Rule::MissingErrorTest => write!(f, "missing-error-test"),
            Rule::MissingBoundaryTest => write!(f, "missing-boundary-test"),
            Rule::SharedState => write!(f, "shared-state"),
            Rule::HardcodedValues => write!(f, "hardcoded-values"),
            Rule::NoAssertions => write!(f, "no-assertions"),
            Rule::SkippedTest => write!(f, "skipped-test"),
            Rule::EmptyTest => write!(f, "empty-test"),
            Rule::DuplicateTest => write!(f, "duplicate-test"),
            Rule::LimitedInputVariety => write!(f, "limited-input-variety"),
            Rule::DebugCode => write!(f, "debug-code"),
            Rule::FocusedTest => write!(f, "focused-test"),
            Rule::FlakyPattern => write!(f, "flaky-pattern"),
            Rule::MockAbuse => write!(f, "mock-abuse"),
            Rule::SnapshotOveruse => write!(f, "snapshot-overuse"),
            Rule::VagueTestName => write!(f, "vague-test-name"),
            Rule::MissingAwait => write!(f, "missing-await"),
            Rule::RtlPreferScreen => write!(f, "rtl-prefer-screen"),
            Rule::RtlPreferSemantic => write!(f, "rtl-prefer-semantic"),
            Rule::RtlPreferUserEvent => write!(f, "rtl-prefer-user-event"),
            Rule::MutationResistant => write!(f, "mutation-resistant"),
            Rule::BoundarySpecificity => write!(f, "boundary-specificity"),
            Rule::StateVerification => write!(f, "state-verification"),
            Rule::AssertionIntentMismatch => write!(f, "assertion-intent-mismatch"),
            Rule::TrivialAssertion => write!(f, "trivial-assertion"),
            Rule::ReturnPathCoverage => write!(f, "return-path-coverage"),
            Rule::BehavioralCompleteness => write!(f, "behavioral-completeness"),
            Rule::SideEffectNotVerified => write!(f, "side-effect-not-verified"),
            Rule::TestComplexity => write!(f, "test-complexity"),
            Rule::ImplementationCoupling => write!(f, "implementation-coupling"),
            Rule::VacuousTest => write!(f, "vacuous-test"),
            Rule::IncompleteMockVerification => write!(f, "incomplete-mock-verification"),
            Rule::AsyncErrorMishandling => write!(f, "async-error-mishandling"),
            Rule::RedundantTest => write!(f, "redundant-test"),
            Rule::UnreachableTestCode => write!(f, "unreachable-test-code"),
            Rule::ExcessiveSetup => write!(f, "excessive-setup"),
            Rule::TypeAssertionAbuse => write!(f, "type-assertion-abuse"),
            Rule::MissingCleanup => write!(f, "missing-cleanup"),
            Rule::TautologicalAssertion => write!(f, "ai-smell-tautological-assertion"),
            Rule::OverMocking => write!(f, "ai-smell-over-mocking"),
            Rule::ShallowVariety => write!(f, "ai-smell-shallow-variety"),
            Rule::HappyPathOnly => write!(f, "ai-smell-happy-path-only"),
            Rule::ParrotAssertion => write!(f, "ai-smell-parrot-assertion"),
            Rule::BoilerplatePadding => write!(f, "ai-smell-boilerplate-padding"),
        }
    }
}

/// Location in a source file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
    /// End line (optional)
    pub end_line: Option<usize>,
    /// End column (optional)
    pub end_column: Option<usize>,
}

impl Location {
    pub fn new(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
            end_line: None,
            end_column: None,
        }
    }

    pub fn with_end(mut self, end_line: usize, end_column: usize) -> Self {
        self.end_line = Some(end_line);
        self.end_column = Some(end_column);
        self
    }
}

/// Detected test framework
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestFramework {
    Jest,
    Vitest,
    Playwright,
    Cypress,
    Mocha,
    Unknown,
}

impl std::fmt::Display for TestFramework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestFramework::Jest => write!(f, "Jest"),
            TestFramework::Vitest => write!(f, "Vitest"),
            TestFramework::Playwright => write!(f, "Playwright"),
            TestFramework::Cypress => write!(f, "Cypress"),
            TestFramework::Mocha => write!(f, "Mocha"),
            TestFramework::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Type of test (affects scoring weights)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TestType {
    /// Unit tests - test individual functions/modules
    #[default]
    Unit,
    /// End-to-end tests - test full user flows
    E2e,
    /// Component tests - test UI components
    Component,
    /// Integration tests - test multiple modules together
    Integration,
}

impl std::fmt::Display for TestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestType::Unit => write!(f, "Unit"),
            TestType::E2e => write!(f, "E2E"),
            TestType::Component => write!(f, "Component"),
            TestType::Integration => write!(f, "Integration"),
        }
    }
}

/// Scoring weights for different test types
/// Each value is a percentage (0-100) that determines category contribution
#[derive(Debug, Clone, Copy)]
pub struct ScoringWeights {
    pub assertion_quality: u8,
    pub error_coverage: u8,
    pub boundary_conditions: u8,
    pub test_isolation: u8,
    pub input_variety: u8,
    pub ai_smells: u8,
}

impl ScoringWeights {
    /// Get default weights for a test type (must sum to 100)
    pub fn for_test_type(test_type: TestType) -> Self {
        match test_type {
            TestType::Unit => Self {
                assertion_quality: 20,
                error_coverage: 15,
                boundary_conditions: 20,
                test_isolation: 15,
                input_variety: 15,
                ai_smells: 15,
            },
            TestType::E2e => Self {
                assertion_quality: 30,
                error_coverage: 15,
                boundary_conditions: 5,
                test_isolation: 25,
                input_variety: 20,
                ai_smells: 5,
            },
            TestType::Component => Self {
                assertion_quality: 25,
                error_coverage: 15,
                boundary_conditions: 15,
                test_isolation: 20,
                input_variety: 20,
                ai_smells: 5,
            },
            TestType::Integration => Self {
                assertion_quality: 22,
                error_coverage: 18,
                boundary_conditions: 15,
                test_isolation: 20,
                input_variety: 20,
                ai_smells: 5,
            },
        }
    }

    /// Calculate total score with these weights
    pub fn calculate_total(&self, breakdown: &ScoreBreakdown) -> u8 {
        let weighted_sum = (breakdown.assertion_quality as u32 * self.assertion_quality as u32)
            + (breakdown.error_coverage as u32 * self.error_coverage as u32)
            + (breakdown.boundary_conditions as u32 * self.boundary_conditions as u32)
            + (breakdown.test_isolation as u32 * self.test_isolation as u32)
            + (breakdown.input_variety as u32 * self.input_variety as u32)
            + (breakdown.ai_smells as u32 * self.ai_smells as u32);
        (weighted_sum / 25).min(100) as u8
    }
}

/// Per-test score for drill-down and "weakest tests first" display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestScore {
    /// Test name (e.g. from it('name', ...))
    pub name: String,
    /// Line range (1-indexed)
    pub line: usize,
    pub end_line: Option<usize>,
    /// Score 0-100 for this test
    pub score: u8,
    /// Letter grade
    pub grade: Grade,
    /// Issues found in this test only
    pub issues: Vec<Issue>,
}

/// Returns true if an issue's location falls within a test's line range
pub fn issue_in_test_range(issue: &Issue, test_line: usize, test_end_line: Option<usize>) -> bool {
    let end = test_end_line.unwrap_or(test_line);
    (issue.location.line >= test_line) && (issue.location.line <= end)
}

/// Statistics about a test file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestStats {
    /// Total number of test cases
    pub total_tests: usize,
    /// Number of skipped tests
    pub skipped_tests: usize,
    /// Total number of assertions
    pub total_assertions: usize,
    /// Number of describe blocks
    pub describe_blocks: usize,
    /// Number of async tests
    pub async_tests: usize,
    /// Function coverage metrics (if source file available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_coverage: Option<FunctionCoverage>,
}

/// Function coverage metrics - what percentage of source exports are tested
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCoverage {
    /// Total number of exports in the source file
    pub total_exports: usize,
    /// Number of exports that appear to be tested
    pub covered_exports: usize,
    /// Coverage percentage (0-100)
    pub coverage_percent: u8,
    /// List of export names that are not referenced in tests
    pub untested_exports: Vec<String>,
    /// List of export names that are referenced in tests
    pub tested_exports: Vec<String>,
}

/// A test case extracted from a test file
#[derive(Debug, Clone)]
pub struct TestCase {
    /// Name of the test
    pub name: String,
    /// Location in the file
    pub location: Location,
    /// Whether the test is async
    pub is_async: bool,
    /// Whether the test is skipped
    pub is_skipped: bool,
    /// Assertions in this test
    pub assertions: Vec<Assertion>,
    /// Parent describe block (if any)
    pub describe_block: Option<String>,
}

/// An assertion extracted from a test
#[derive(Debug, Clone)]
pub struct Assertion {
    /// The kind of assertion
    pub kind: AssertionKind,
    /// Quality classification
    pub quality: AssertionQuality,
    /// Location in the file
    pub location: Location,
    /// Raw assertion text
    pub raw: String,
}

/// Types of assertions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssertionKind {
    /// expect(x).toBe(y)
    ToBe,
    /// expect(x).toEqual(y)
    ToEqual,
    /// expect(x).toStrictEqual(y)
    ToStrictEqual,
    /// expect(x).toBeDefined()
    ToBeDefined,
    /// expect(x).toBeUndefined()
    ToBeUndefined,
    /// expect(x).toBeNull()
    ToBeNull,
    /// expect(x).toBeTruthy()
    ToBeTruthy,
    /// expect(x).toBeFalsy()
    ToBeFalsy,
    /// expect(x).toThrow()
    ToThrow,
    /// expect(x).toHaveBeenCalled()
    ToHaveBeenCalled,
    /// expect(x).toContain(y)
    ToContain,
    /// expect(x).toMatch(y)
    ToMatch,
    /// expect(x).toHaveLength(n)
    ToHaveLength,
    /// expect(x).toBeGreaterThan(y)
    ToBeGreaterThan,
    /// expect(x).toBeLessThan(y)
    ToBeLessThan,
    /// expect(x).toHaveProperty(k, v)
    ToHaveProperty,
    /// expect(x).toMatchSnapshot()
    ToMatchSnapshot,
    /// expect(x).toMatchInlineSnapshot()
    ToMatchInlineSnapshot,
    /// expect(x).toHaveBeenCalledTimes(n)
    ToHaveBeenCalledTimes,
    /// expect(x).toHaveBeenNthCalledWith(n, ...)
    ToHaveBeenNthCalledWith,
    /// expect(x).toBeInstanceOf(Class)
    ToBeInstanceOf,
    /// expect(x).toHaveClass(name) - Testing Library
    ToHaveClass,
    /// expect(x).toBeVisible() - Playwright
    ToBeVisible,
    /// expect(x).toHaveText(text) - Playwright
    ToHaveText,
    /// cy.get().should('exist') - Cypress
    CyShouldExist,
    /// cy.get().should('be.visible') - Cypress
    CyShouldBeVisible,
    /// cy.get().should('have.text', x) - Cypress
    CyShouldHaveText,
    /// cy.get().should('contain', x) - Cypress
    CyShouldContain,
    /// cy.get().should('have.length', n) - Cypress
    CyShouldHaveLength,
    /// cy.get().should('eq', x) - Cypress
    CyShouldEqual,
    /// cy.get().should('be.disabled') - Cypress
    CyShouldBeDisabled,
    /// cy.get().should('have.attr', k, v) - Cypress
    CyShouldHaveAttr,
    /// cy.contains() - implicit assertion - Cypress (Moderate quality)
    CyContains,
    /// cy.url().should() - URL assertion - Cypress (Moderate quality)
    CyUrl,
    /// cy.intercept() - Network interception - Cypress (Moderate quality)
    CyIntercept,
    /// cy.get() without .should() - Weak implicit assertion (wait/existence)
    CyGetImplicit,
    /// cy.visit() - Navigation, weak implicit assertion
    CyVisit,
    /// cy.click(), cy.type(), etc. - Actions implying element exists
    CyAction,
    /// assert.* style
    Assert,
    /// Negated assertion (expect(x).not.*)
    Negated(Box<AssertionKind>),
    /// Unknown assertion type
    Unknown(String),
}

/// Quality classification of an assertion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssertionQuality {
    /// Strong assertion (toBe, toEqual, toThrow with message)
    Strong,
    /// Moderate assertion (toContain, toMatch)
    Moderate,
    /// Weak assertion (toBeDefined, toBeTruthy)
    Weak,
    /// No real assertion value
    None,
}

impl AssertionKind {
    pub fn quality(&self) -> AssertionQuality {
        match self {
            // Strong assertions - check specific values
            AssertionKind::ToBe
            | AssertionKind::ToEqual
            | AssertionKind::ToStrictEqual
            | AssertionKind::ToThrow
            | AssertionKind::ToHaveProperty
            | AssertionKind::ToBeGreaterThan
            | AssertionKind::ToBeLessThan
            | AssertionKind::ToHaveBeenCalledTimes
            | AssertionKind::ToHaveBeenNthCalledWith
            | AssertionKind::ToHaveText
            | AssertionKind::CyShouldHaveText
            | AssertionKind::CyShouldHaveLength
            | AssertionKind::CyShouldEqual
            | AssertionKind::CyShouldHaveAttr => AssertionQuality::Strong,

            // Moderate assertions - check partial values
            AssertionKind::ToContain
            | AssertionKind::ToMatch
            | AssertionKind::ToHaveLength
            | AssertionKind::ToHaveBeenCalled
            | AssertionKind::Assert
            | AssertionKind::ToBeInstanceOf
            | AssertionKind::ToHaveClass
            | AssertionKind::ToBeVisible
            | AssertionKind::CyShouldBeVisible
            | AssertionKind::CyShouldContain
            | AssertionKind::CyShouldBeDisabled
            | AssertionKind::CyContains
            | AssertionKind::CyUrl
            | AssertionKind::CyIntercept => AssertionQuality::Moderate,

            // Weak - snapshot assertions don't verify specific behavior
            AssertionKind::ToMatchSnapshot | AssertionKind::ToMatchInlineSnapshot => {
                AssertionQuality::Weak
            }

            // Weak assertions - only check existence/truthiness
            AssertionKind::ToBeDefined
            | AssertionKind::ToBeUndefined
            | AssertionKind::ToBeNull
            | AssertionKind::ToBeTruthy
            | AssertionKind::ToBeFalsy
            | AssertionKind::CyShouldExist
            | AssertionKind::CyGetImplicit
            | AssertionKind::CyVisit
            | AssertionKind::CyAction => AssertionQuality::Weak,

            // Negated assertions: preserve Strong for not.toHaveBeenCalled(), not.toThrow(), not.toBe(), not.toEqual()
            AssertionKind::Negated(inner) => {
                let inner_quality = inner.quality();
                let preserves_strong = matches!(
                    **inner,
                    AssertionKind::ToHaveBeenCalled
                        | AssertionKind::ToThrow
                        | AssertionKind::ToBe
                        | AssertionKind::ToEqual
                );
                if preserves_strong {
                    AssertionQuality::Strong
                } else {
                    match inner_quality {
                        AssertionQuality::Strong => AssertionQuality::Moderate,
                        AssertionQuality::Moderate => AssertionQuality::Weak,
                        _ => AssertionQuality::Weak,
                    }
                }
            }

            AssertionKind::Unknown(_) => AssertionQuality::None,
        }
    }
}

/// Public API: analyze a single test file. Used by LSP and other programmatic consumers.
///
/// * `path` - path to the test file
/// * `work_dir` - project root (for config lookup and source mapping)
/// * `config_path` - optional path to .rigorrc.json; if None, searches from work_dir
pub fn analyze_file(
    path: &std::path::Path,
    work_dir: &std::path::Path,
    config_path: Option<&std::path::Path>,
) -> anyhow::Result<AnalysisResult> {
    let config = crate::config::load_config(work_dir, config_path).ok();
    let engine =
        crate::analyzer::engine::AnalysisEngine::new().with_project_root(work_dir.to_path_buf());
    engine.analyze(path, config.as_ref())
}
