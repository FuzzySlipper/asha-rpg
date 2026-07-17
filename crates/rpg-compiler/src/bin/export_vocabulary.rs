use rpg_compiler::{capability_registrations, operation_registrations};
use rpg_ir::{RPG_IR_IDENTITY, RPG_IR_MAJOR};
use serde_json::json;

fn main() {
    let operations = operation_registrations()
        .iter()
        .map(|registration| {
            json!({
                "id": registration.id,
                "version": registration.version,
                "reads": registration.reads.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
                "mutationOwner": registration.mutation_owner.as_str(),
                "validationBehavior": registration.validation_behavior,
                "acceptedEvents": registration.accepted_events,
                "traceBehavior": registration.trace_behavior,
                "replayImplications": registration.replay_implications,
            })
        })
        .collect::<Vec<_>>();
    let capabilities = capability_registrations()
        .iter()
        .map(|registration| {
            json!({
                "id": registration.id.as_str(),
                "version": registration.version,
            })
        })
        .collect::<Vec<_>>();
    let vocabulary = json!({
        "identity": RPG_IR_IDENTITY,
        "major": RPG_IR_MAJOR,
        "operations": operations,
        "capabilities": capabilities,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&vocabulary).expect("static RPG vocabulary serializes")
    );
}
