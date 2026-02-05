//! Analysis rules for test quality

pub mod assertion_intent;
pub mod assertion_quality;
pub mod async_patterns;
pub mod behavioral_completeness;
pub mod boundary_conditions;
pub mod boundary_specificity;
pub mod coupling;
pub mod debug_code;
pub mod error_coverage;
pub mod flaky_patterns;
pub mod input_variety;
pub mod mock_abuse;
pub mod mutation_resistant;
pub mod naming_quality;
pub mod react_testing_library;
pub mod return_path_coverage;
pub mod side_effect_verification;
pub mod state_verification;
pub mod test_isolation;
pub mod trivial_assertion;

pub use assertion_intent::AssertionIntentRule;
pub use assertion_quality::AssertionQualityRule;
pub use async_patterns::AsyncPatternsRule;
pub use behavioral_completeness::BehavioralCompletenessRule;
pub use boundary_conditions::BoundaryConditionsRule;
pub use boundary_specificity::BoundarySpecificityRule;
pub use coupling::CouplingAnalysisRule;
pub use debug_code::DebugCodeRule;
pub use error_coverage::ErrorCoverageRule;
pub use flaky_patterns::FlakyPatternsRule;
pub use input_variety::InputVarietyRule;
pub use mock_abuse::MockAbuseRule;
pub use mutation_resistant::MutationResistantRule;
pub use naming_quality::NamingQualityRule;
pub use react_testing_library::ReactTestingLibraryRule;
pub use return_path_coverage::ReturnPathCoverageRule;
pub use side_effect_verification::SideEffectVerificationRule;
pub use state_verification::StateVerificationRule;
pub use test_isolation::TestIsolationRule;
pub use trivial_assertion::TrivialAssertionRule;

use crate::{Issue, TestCase};
use tree_sitter::Tree;

/// Trait for analysis rules
pub trait AnalysisRule {
    /// Name of the rule
    fn name(&self) -> &'static str;

    /// Analyze test cases and return issues found
    fn analyze(&self, tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue>;

    /// Calculate score for this category (0-25)
    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8;
}
