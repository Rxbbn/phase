//! "<object> loses this ability" removes exactly that one printed activated
//! ability, permanently, from exactly that one object.
//!
//! CR 613.1f: layer 6 applies ability-removing effects.
//! CR 611.2a: neither printed text states a duration, so the continuous effect
//!            lasts until the end of the game (`Duration::Permanent`).
//! CR 608.2m: the activated ability that removes itself still finishes
//!            resolving — the Conjure / the Aura animation and attach all run.
//! CR 611.2c: the set of objects a resolution-created continuous effect affects
//!            is fixed when it begins, so a second copy of the same card that
//!            did not activate keeps its ability.
//!
//! Two cards, two shapes of the same clause:
//!
//! * **Great Fang Chroniclers** — "{3}{G}: Conjure a card named Muraganda
//!   Petroglyphs onto the battlefield, then this creature loses this ability.
//!   Activate only as a sorcery." The removal is the whole tail, and the cost
//!   is mana-only, so a second activation is illegal *only* because the ability
//!   is gone — the discriminating assertion of this file.
//!
//!   DEFERRED(Unit 2): the verbatim card writes the tail as an intra-sentence
//!   ", then …" continuation, and `split_comma_clause_boundary`'s `then `
//!   branch does not yet cut there (the subject-headed continuation arm is a
//!   separate change). The fixture below therefore writes the same two
//!   instructions as two sentences — the form the chunker already splits — so
//!   the tail reaches the clause grammar. Everything downstream of that cut is
//!   the production path. When the splitter lands, this constant becomes the
//!   verbatim Oracle text with no other change.
//!
//! * **Calming Licid** — verbatim Oracle text, no deferral: "{W}, {T}: This
//!   creature loses this ability and becomes an Aura enchantment with enchant
//!   creature. …" The clause is already its own sentence, so the real card
//!   exercises the new modification today — including CR 116.2c's "You may pay
//!   {W} to end this effect", which with a `Permanent` duration is the only way
//!   the loss ever ends.
//!
//! Flanking Licid prints the same predicate and gets the same modification,
//! but its clause also states a type-set exclusion the animation grammar cannot
//! perform ("… that reads \"Enchanted creature gains flanking\" instead of a
//! creature"), so the CR 611.2a `Permanent` promotion is withheld from it and it
//! keeps the duration it had before this unit. Covered by
//! `flanking_licid_type_exclusion_is_not_promoted_to_permanent` in
//! `parser/oracle_effect/tests.rs`.

use crate::support::shared_card_db;
use engine::ai_support::legal_actions_for_viewer;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::{ContinuousModification, Duration};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

/// DEFERRED(Unit 2) — see the module docs. Identical to the printed card except
/// that the ", then" continuation is written as its own sentence.
const GREAT_FANG_CHRONICLERS: &str = "Double team\n\
{3}{G}: Conjure a card named Muraganda Petroglyphs onto the battlefield. \
Then this creature loses this ability. Activate only as a sorcery.";

/// Verbatim Oracle text.
const CALMING_LICID: &str = "{W}, {T}: This creature loses this ability and becomes an Aura \
enchantment with enchant creature. Attach it to target creature. You may pay {W} to end this \
effect.\nEnchanted creature can't attack.";

fn mana(color: ManaType, count: usize) -> Vec<ManaUnit> {
    (0..count)
        .map(|_| ManaUnit::new(color, ObjectId(0), false, vec![]))
        .collect()
}

/// The index of `source`'s one costed activated ability, or `None` when it has
/// none left.
fn costed_ability(runner: &GameRunner, source: ObjectId) -> Option<usize> {
    runner.state().objects[&source]
        .abilities
        .iter()
        .position(|a| a.cost.is_some())
}

/// Is activating `source`'s ability `index` offered on the production legality
/// surface the AI and the client both read?
fn activation_is_legal(runner: &GameRunner, source: ObjectId, index: usize) -> bool {
    engine::ai_support::legal_actions(runner.state())
        .iter()
        .any(|action| {
            matches!(
                action,
                GameAction::ActivateAbility {
                    source_id,
                    ability_index,
                    ..
                } if *source_id == source && *ability_index == index
            )
        })
}

fn battlefield_has_named(runner: &GameRunner, name: &str) -> bool {
    runner
        .state()
        .objects
        .values()
        .any(|o| o.zone == Zone::Battlefield && o.name == name)
}

/// Build the Great Fang Chroniclers board: the activator, a second
/// non-activating copy (the multi-authority hostile fixture), and enough mana
/// for TWO activations so affordability can never be the reason the second one
/// is refused.
fn chroniclers_board() -> Option<(GameRunner, ObjectId, ObjectId)> {
    let db = shared_card_db()?;
    let petroglyphs = db.get_face_by_name("Muraganda Petroglyphs")?.clone();

    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, mana(ManaType::Green, 8));
    let activator = scenario
        .add_creature(P0, "Great Fang Chroniclers", 2, 2)
        .from_oracle_text_with_keywords(&["Double team"], GREAT_FANG_CHRONICLERS)
        .id();
    let bystander = scenario
        .add_creature(P0, "Great Fang Chroniclers", 2, 2)
        .from_oracle_text_with_keywords(&["Double team"], GREAT_FANG_CHRONICLERS)
        .id();
    let mut runner = scenario.build();
    // Conjure reads the outside-the-game face registry; a scenario-built card
    // carries no printed ref, so seed it exactly as production rehydration does.
    std::sync::Arc::make_mut(&mut runner.state_mut().card_face_registry)
        .insert("muraganda petroglyphs".to_string(), petroglyphs);
    Some((runner, activator, bystander))
}

/// THE DISCRIMINATING TEST. Activating the printed ability conjures, then
/// removes that ability from the activating permanent only — so a second
/// activation is no longer offered, while the identical second copy's is.
///
/// Revert probe: drop the `RemoveSourceAbility` apply arm in `layers.rs` (or the
/// parser arm that emits it) and the "second activation is gone" assertion
/// fails. Make the apply arm behave like `RemoveAllAbilities` and the Double
/// team rows fail.
#[test]
fn great_fang_chroniclers_loses_only_its_own_activated_ability() {
    let Some((mut runner, activator, bystander)) = chroniclers_board() else {
        eprintln!("skipping: card-data fixture not generated");
        return;
    };
    let index = costed_ability(&runner, activator).expect("the printed {3}{G} ability");
    let bystander_index = costed_ability(&runner, bystander).expect("the printed {3}{G} ability");

    // Reach-guards: BOTH copies' abilities are activatable before anything runs,
    // so neither later negative can pass vacuously on cost or timing.
    assert!(activation_is_legal(&runner, activator, index));
    assert!(activation_is_legal(&runner, bystander, bystander_index));

    let outcome = runner.activate(activator, index).resolve();
    let conjured =
        outcome.find_object(|o| o.zone == Zone::Battlefield && o.name == "Muraganda Petroglyphs");
    assert!(
        conjured.is_some(),
        "CR 608.2m: the ability finishes resolving — the Conjure happened"
    );
    runner.advance_until_stack_empty();
    assert!(battlefield_has_named(&runner, "Muraganda Petroglyphs"));

    // THE LOAD-BEARING ASSERTION (CR 613.1f): the ability removed itself, read
    // off the production legality surface rather than the ability vector.
    assert!(
        !activation_is_legal(&runner, activator, index),
        "CR 613.1f: the activated ability must be gone after it removes itself"
    );
    // PAIRED POSITIVE (CR 611.2c): the second, non-activating copy keeps its
    // ability — the removal is bound to one object, not to the card. This also
    // proves the mana and the sorcery-timing window are still satisfied, so the
    // negative above is about the ability's absence and nothing else.
    assert!(
        activation_is_legal(&runner, bystander, bystander_index),
        "CR 611.2c: the effect's affected set is the one object it named"
    );

    // PAIRED POSITIVE (surgical, not a wipe): Double team and its attack
    // trigger are untouched.
    let obj = &runner.state().objects[&activator];
    assert!(
        obj.keywords.contains(&Keyword::DoubleTeam),
        "RemoveSourceAbility must not behave like RemoveAllAbilities, got {:?}",
        obj.keywords
    );
    assert!(
        !obj.trigger_definitions.is_empty(),
        "the Double team attack trigger must survive the removal"
    );
}

/// CR 611.2a: the removal states no duration, so it lasts until the end of the
/// game — the installed effect must carry `Duration::Permanent`, and the loss
/// must still hold after the turn that created it ends (CR 514.2 ends every
/// "until end of turn" effect at that turn's cleanup step).
///
/// Revert probe (run): disable the `Duration::Permanent` default in
/// `build_continuous_clause` and the installed effect comes out
/// `UntilEndOfTurn` — the first assertion below fails. The turn-crossing half
/// needs the explicit empty attack declaration: `advance_to_phase` stops at the
/// first non-priority prompt, so without it the walk parks at
/// `WaitingFor::DeclareAttackers` inside the SAME turn and the final assertion
/// would pass without ever reaching a cleanup step.
#[test]
fn great_fang_chroniclers_ability_loss_outlives_the_turn() {
    let Some((mut runner, activator, _bystander)) = chroniclers_board() else {
        eprintln!("skipping: card-data fixture not generated");
        return;
    };
    let index = costed_ability(&runner, activator).expect("the printed {3}{G} ability");
    runner.activate(activator, index).resolve();
    runner.advance_until_stack_empty();
    assert!(
        costed_ability(&runner, activator).is_none(),
        "reach: the removal applied this turn"
    );

    // CR 611.2a, read off the installed effect itself.
    assert!(
        runner
            .state()
            .transient_continuous_effects
            .iter()
            .any(|tce| {
                matches!(tce.duration, Duration::Permanent)
                    && tce
                        .modifications
                        .iter()
                        .any(|m| matches!(m, ContinuousModification::RemoveSourceAbility { .. }))
            }),
        "CR 611.2a: a removal stating no duration installs a Permanent effect, got {:?}",
        runner
            .state()
            .transient_continuous_effects
            .iter()
            .map(|tce| (tce.duration.clone(), tce.modifications.clone()))
            .collect::<Vec<_>>()
    );

    // Past this turn's cleanup (CR 514.2) and into the next turn. The empty
    // declaration is what lets the phase walk leave combat.
    let turn_before = runner.state().turn_number;
    runner.advance_to_phase(Phase::DeclareAttackers);
    runner
        .declare_attackers(&[])
        .expect("declaring no attackers is legal");
    runner.advance_to_phase(Phase::Upkeep);
    assert!(
        runner.state().turn_number > turn_before,
        "reach: the fixture actually crossed the turn boundary (turn {} -> {}, phase {:?})",
        turn_before,
        runner.state().turn_number,
        runner.state().phase
    );
    assert!(
        costed_ability(&runner, activator).is_none(),
        "CR 611.2a: no stated duration means the loss lasts until end of game"
    );
}

/// Verbatim Calming Licid: the same primitive, reached from a shipped card's
/// own Oracle text with no deferral. The Licid's cost taps it, so legality
/// alone cannot discriminate here; the assertion is on the post-layer ability
/// set the activation pipeline produced, paired with the animation reach-guard
/// and the second-copy hostile fixture.
#[test]
fn calming_licid_loses_its_own_ability_when_it_becomes_an_aura() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, mana(ManaType::White, 4));
    let licid = scenario
        .add_creature_from_oracle(P0, "Calming Licid", 1, 1, CALMING_LICID)
        .id();
    let other_licid = scenario
        .add_creature_from_oracle(P0, "Calming Licid", 1, 1, CALMING_LICID)
        .id();
    let host = scenario.add_creature(P1, "Grizzly Bears", 2, 2).id();
    let mut runner = scenario.build();

    let index = costed_ability(&runner, licid).expect("the printed {W}, {T} ability");
    let outcome = runner
        .activate(licid, index)
        .target_objects(&[host])
        .resolve();

    // Reach-guard: the rest of the clause ran (CR 608.2m) — the Licid is an
    // Aura enchantment now, so the assertion below is not reading an object the
    // effect never touched.
    let animated = &outcome.state().objects[&licid];
    assert!(
        animated
            .card_types
            .core_types
            .contains(&CoreType::Enchantment)
            && animated.card_types.subtypes.iter().any(|s| s == "Aura"),
        "reach: the animation applied, got {:?}",
        animated.card_types
    );
    // CR 613.1f: its own activated ability is gone.
    assert!(
        animated.abilities.is_empty(),
        "the Licid must lose its activated ability, got {:?}",
        animated.abilities
    );
    // CR 611.2c: the other Licid, which did not activate, keeps its ability.
    assert_eq!(
        outcome.state().objects[&other_licid].abilities.len(),
        1,
        "the removal is bound to the one object the effect named"
    );
    drop(outcome);

    // CR 116.2c: "You may pay {W} to end this effect" is a special action the
    // controller may take any time they have priority — and with the installed
    // effect now `Duration::Permanent` (CR 611.2a) it is the ONLY way the loss
    // ever ends. Taken through the production legality surface, never
    // hand-constructed, so a missing enumeration entry fails here.
    let offered: Vec<GameAction> = legal_actions_for_viewer(runner.state(), P0)
        .0
        .into_iter()
        .filter(|action| matches!(action, GameAction::EndContinuousEffect { .. }))
        .collect();
    assert_eq!(
        offered.len(),
        1,
        "CR 116.2c: exactly one pay-to-end permission must be offered, got {offered:?}"
    );
    runner
        .act(offered[0].clone())
        .expect("the offered pay-to-end special action must be legal");
    assert_eq!(
        costed_ability(&runner, licid),
        Some(0),
        "CR 116.2c: ending the effect restores the removed activated ability"
    );
    // Reach-guard on the same action: the animation it shared the effect with
    // also ended, so the restoration is the whole effect expiring rather than
    // the removal alone being special-cased.
    assert!(
        !runner.state().objects[&licid]
            .card_types
            .core_types
            .contains(&CoreType::Enchantment),
        "reach: the pay-to-end action ended the whole continuous effect"
    );
}
