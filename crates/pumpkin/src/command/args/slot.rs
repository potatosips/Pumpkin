use pumpkin_data::slot_ranges::get_slot_range;
use pumpkin_protocol::java::client::play::{ArgumentType, SuggestionProviders};

use crate::{
    command::{CommandSender, args::ConsumeResult, dispatcher::CommandError, tree::RawArgs},
    server::Server,
};

use super::{Arg, ArgumentConsumer, DefaultNameArgConsumer, FindArg, GetClientSideArgParser};

pub struct SlotArgumentConsumer;

impl GetClientSideArgParser for SlotArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::ItemSlot
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

impl ArgumentConsumer for SlotArgumentConsumer {
    fn consume<'a>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let slot = args.pop().map(|arg| arg.value);
        match slot {
            Some(s) => Box::pin(async move {
                if let Some(range) = get_slot_range(s)
                    && range.len() == 1
                {
                    return Some(Arg::Slot(range[0], s.to_string()));
                }
                None
            }),
            None => Box::pin(async move { None }),
        }
    }
}

impl DefaultNameArgConsumer for SlotArgumentConsumer {
    fn default_name(&self) -> &'static str {
        "slot"
    }
}

impl<'a> FindArg<'a> for SlotArgumentConsumer {
    type Data = (usize, String);

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::Slot(slot, name)) => Ok((*slot, name.clone())),
            _ => Err(CommandError::InvalidConsumption(Some(name.to_string()))),
        }
    }
}

pub struct SlotsArgumentConsumer;

impl GetClientSideArgParser for SlotsArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::ItemSlots
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

impl ArgumentConsumer for SlotsArgumentConsumer {
    fn consume<'a>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let slots = args.pop().map(|arg| arg.value);
        match slots {
            Some(s) => Box::pin(async move {
                if let Some(range) = get_slot_range(s) {
                    return Some(Arg::Slots(range, s.to_string()));
                }
                None
            }),
            None => Box::pin(async move { None }),
        }
    }
}

impl DefaultNameArgConsumer for SlotsArgumentConsumer {
    fn default_name(&self) -> &'static str {
        "slots"
    }
}

impl<'a> FindArg<'a> for SlotsArgumentConsumer {
    type Data = (&'static [usize], String);

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::Slots(slots, name)) => Ok((*slots, name.clone())),
            _ => Err(CommandError::InvalidConsumption(Some(name.to_string()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_slot_and_slot_ranges_lookup_parity() {
        let single_slot = get_slot_range("weapon.mainhand");
        assert!(single_slot.is_some());
        assert_eq!(single_slot.unwrap().len(), 1);

        let armor_head = get_slot_range("armor.head");
        assert!(armor_head.is_some());
        assert_eq!(armor_head.unwrap().len(), 1);

        let multi_slots = get_slot_range("container.*");
        assert!(multi_slots.is_some());
        assert!(multi_slots.unwrap().len() > 1);

        let invalid_slot = get_slot_range("invalid.slot.identifier");
        assert!(invalid_slot.is_none());
    }
}
