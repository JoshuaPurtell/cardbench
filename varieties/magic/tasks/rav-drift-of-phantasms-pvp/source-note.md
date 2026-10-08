# Drift of Phantasms source note (RAV #46)

The RAV catalog identifies one printing as Drift of Phantasms, collector number 46. The printed identity is a blue `{2}{U}` Creature — Spirit with defender, flying, Transmute `{1}{U}{U}`, and 0/5 characteristics. The RAV Transmute reminder uses “converted mana cost,” says to reveal the selected card and put it into hand, then shuffle the library, and permits play only as a sorcery.

Public RAV record: <https://mtg.wtf/card/rav/46/Drift-of-Phantasms>. The printed RAV rules text is retained from the RAV row at <https://manabase.com/card.php?name=Drift+of+Phantasms>. The catalog remains the authority for the set identity and one-printing denominator.

Current Oracle changes “converted mana cost” to “mana value” and says “Transmute only as a sorcery.” The cost, same-mana-value search, reveal-to-hand destination, shuffle, and sorcery timing are unchanged. The task follows `varieties/magic/tasks/rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md`: the RAV printing controls identity and printed mechanics; Oracle is used only to interpret the terminology update.

The task retains the printed Spirit subtype as public identity, but the current `CardDefinition` model stores only `CardType::Creature`; it makes no subtype assertion. The executable definition and gameplay trace check the blue `{2}{U}` cost, 0/5 characteristics, Defender, Flying, and Transmute `{1}{U}{U}`. No art, scan, flavor, price, or set-context fact is used as a mechanic.
