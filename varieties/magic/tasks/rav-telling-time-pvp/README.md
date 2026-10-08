# Telling Time RAV card task

Implement the candidate-owned module for RAV #69, Telling Time. It is a `{1}{U}` blue instant. When it resolves, its controller privately looks at the top three cards of their library, puts one into their hand, one back on top, and one on the bottom.

The trusted authority runs a shuffled, legal 60-card PvP match. It requires the controller's view to contain the ordered private snapshot of exactly three cards, confirms that the opponent's view contains neither the decision nor any candidate identity, and checks the exhaustive hand/top/bottom choice against the post-resolution zones and library order. The same match continues to a seat-zero Watchwolf combat win, and a replay with the same seed must reproduce the captured events and partition. Cost, color, and effect mutations must be rejected. The task awards no set credit or Harbor reward.
