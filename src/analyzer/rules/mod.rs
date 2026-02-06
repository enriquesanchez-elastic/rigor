//! Analysis rules for test quality

pub mod ai_smells;
pub mod assertion_intent;
pub mod assertion_quality;
pub mod async_error_mishandling;
pub mod async_patterns;
pub mod behavioral_completeness;
pub mod boundary_conditions;
pub mod boundary_specificity;
pub mod coupling;
pub mod debug_code;
pub mod error_coverage;
pub mod excessive_setup;
pub mod flaky_patterns;
pub mod implementation_coupling;
pub mod incomplete_mock_verification;
pub mod input_variety;
pub mod missing_cleanup;
pub mod mock_abuse;
pub mod mutation_resistant;
pub mod naming_quality;
pub mod react_testing_library;
pub mod redundant_test;
pub mod return_path_coverage;
pub mod side_effect_verification;
pub mod state_verification;
pub mod test_complexity;
pub mod test_isolation;
pub mod trivial_assertion;
pub mod type_assertion_abuse;
pub mod unreachable_test_code;
pub mod vacuous_test;

pub use ai_smells::AiSmellsRule;
pub use assertion_intent::AssertionIntentRule;
pub use assertion_quality::AssertionQualityRule;
pub use async_error_mishandling::AsyncErrorMishandlingRule;
pub use async_patterns::AsyncPatternsRule;
pub use behavioral_completeness::BehavioralCompletenessRule;
pub use boundary_conditions::BoundaryConditionsRule;
pub use boundary_specificity::BoundarySpecificityRule;
pub use coupling::CouplingAnalysisRule;
pub use debug_code::DebugCodeRule;
pub use error_coverage::ErrorCoverageRule;
pub use excessive_setup::ExcessiveSetupRule;
pub use flaky_patterns::FlakyPatternsRule;
pub use implementation_coupling::ImplementationCouplingRule;
pub use incomplete_mock_verification::IncompleteMockVerificationRule;
pub use input_variety::InputVarietyRule;
pub use missing_cleanup::MissingCleanupRule;
pub use mock_abuse::MockAbuseRule;
pub use mutation_resistant::MutationResistantRule;
pub use naming_quality::NamingQualityRule;
pub use react_testing_library::ReactTestingLibraryRule;
pub use redundant_test::RedundantTestRule;
pub use return_path_coverage::ReturnPathCoverageRule;
pub use side_effect_verification::SideEffectVerificationRule;
pub use state_verification::StateVerificationRule;
pub use test_complexity::TestComplexityRule;
pub use test_isolation::TestIsolationRule;
pub use trivial_assertion::TrivialAssertionRule;
pub use type_assertion_abuse::TypeAssertionAbuseRule;
pub use unreachable_test_code::UnreachableTestCodeRule;
pub use vacuous_test::VacuousTestRule;

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
