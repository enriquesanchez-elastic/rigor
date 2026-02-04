//! Rigor: Test Quality Analyzer for TypeScript
//!
//! This library provides static analysis of TypeScript test files to evaluate
//! test quality and provide actionable suggestions for improvement.

pub mod analyzer;
pub mod cache;
pub mod config;
pub mod detector;
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
    /// List of issues found
    pub issues: Vec<Issue>,
    /// Statistics about the test file
    pub stats: TestStats,
    /// Detected test framework
    pub framework: TestFramework,
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
}

impl ScoreBreakdown {
    pub fn total(&self) -> u8 {
        // Each category is 0-25, but we have 5 categories
        // Normalize to 0-100 by taking weighted average
        let sum = self.assertion_quality as u16
            + self.error_coverage as u16
            + self.boundary_conditions as u16
            + self.test_isolation as u16
            + self.input_variety as u16;
        // Each category contributes 20 points max to the final score
        ((sum * 100) / 125).min(100) as u8
    }
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
            | AssertionKind::CyShouldBeDisabled => AssertionQuality::Moderate,

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
            | AssertionKind::CyShouldExist => AssertionQuality::Weak,

            // Negated assertions inherit quality but weaken it
            AssertionKind::Negated(inner) => match inner.quality() {
                AssertionQuality::Strong => AssertionQuality::Moderate,
                AssertionQuality::Moderate => AssertionQuality::Weak,
                _ => AssertionQuality::Weak,
            },

            AssertionKind::Unknown(_) => AssertionQuality::None,
        }
    }
}
