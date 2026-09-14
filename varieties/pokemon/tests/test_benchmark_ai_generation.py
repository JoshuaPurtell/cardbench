from pathlib import Path
import unittest


BENCHMARK_SOURCE = (
    Path(__file__).resolve().parents[1] / "policies" / "benchmark_ai.py"
).read_text(encoding="utf-8")


class GeneratedBenchmarkContracts(unittest.TestCase):
    def test_installs_composed_hooks_in_canonical_set_order(self) -> None:
        self.assertIn(
            "game.set_hooks(tcg_expansions::create_hooks_for(&loaded_sets))",
            BENCHMARK_SOURCE,
        )
        self.assertIn("fn deck_order_does_not_change_loaded_sets()", BENCHMARK_SOURCE)
        self.assertIn(
            '[("CG", has_set("CG")), ("DF", has_set("DF")), ("HP", has_set("HP"))]',
            BENCHMARK_SOURCE,
        )

    def test_private_crystal_guardians_is_fail_closed(self) -> None:
        self.assertIn('features = ["cg_private"]', BENCHMARK_SOURCE)
        self.assertIn("private Crystal Guardians module missing", BENCHMARK_SOURCE)

    def test_budget_exhaustion_is_counted_as_a_draw(self) -> None:
        self.assertIn(
            "Some((None, game, p1_evolutions, p2_evolutions))",
            BENCHMARK_SOURCE,
        )
        self.assertIn('"budget_draws": overall_stats.budget_draws', BENCHMARK_SOURCE)
        self.assertIn("stats.budget_draws += 1", BENCHMARK_SOURCE)

    def test_prompt_fallback_completes_generated_matches(self) -> None:
        self.assertIn("p1_prompt_fallback.propose_prompt_response", BENCHMARK_SOURCE)
        self.assertIn("p2_prompt_fallback.propose_prompt_response", BENCHMARK_SOURCE)


if __name__ == "__main__":
    unittest.main()
