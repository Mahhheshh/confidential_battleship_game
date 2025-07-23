use arcis_imports::*;

#[encrypted]
mod circuits {
    use arcis_imports::*;

    pub struct PlayerShipFleet {
        player_1: [[u8; 2]; 17], // Array of [row, col] coordinates for Player 1
        player_2: [[u8; 2]; 17], // Array of [row, col] coordinates for Player 2
    }

    pub struct PlaceShipsInputs {
        is_player_1: bool,
        fleet_location: [[u8; 2]; 17],
    }

    pub struct TakeTurnInputs {
        is_player_1: bool,
        guess: [u8; 2],
    }

    #[instruction]
    pub fn init_player_ship_fleet_location(mxe: Mxe) -> Enc<Mxe, PlayerShipFleet> {
        mxe.from_arcis(PlayerShipFleet {
            player_1: [[255; 2]; 17],
            player_2: [[255; 2]; 17],
        })
    }

    #[instruction]
    pub fn place_ships(
        input_ctxt: Enc<Shared, PlaceShipsInputs>,
        player_ship_fleet_ctxt: Enc<Mxe, PlayerShipFleet>,
    ) -> Enc<Mxe, PlayerShipFleet> {
        let player_inputs = input_ctxt.to_arcis();
        let mut ship_fleet_data = player_ship_fleet_ctxt.to_arcis();

        if player_inputs.is_player_1 {
            ship_fleet_data.player_1 = player_inputs.fleet_location;
        } else {
            ship_fleet_data.player_2 = player_inputs.fleet_location;
        }

        player_ship_fleet_ctxt.owner.from_arcis(ship_fleet_data)
    }

    #[instruction]
    pub fn take_turn(
        input_ctxt: TakeTurnInputs, // this do not have to be encrypted
        player_ship_fleet_ctxt: Enc<Mxe, PlayerShipFleet>,
    ) -> (Enc<Mxe, PlayerShipFleet>, bool) {
        let is_player_1 = input_ctxt.is_player_1;
        let [row, col] = input_ctxt.guess;
        let mut ship_fleet_data = player_ship_fleet_ctxt.to_arcis();

        let mut was_hit = false;

        let enemy_fleet_location = if is_player_1 {
            &mut ship_fleet_data.player_2
        } else {
            &mut ship_fleet_data.player_1
        };
        for i in 0..enemy_fleet_location.len() {
            let ship_loc = enemy_fleet_location[i];
            if ship_loc[0] == row && ship_loc[1] == col {
                was_hit = true;

                enemy_fleet_location[i] = [11, 11];
            }
        }

        (
            player_ship_fleet_ctxt.owner.from_arcis(ship_fleet_data),
            was_hit.reveal(),
        )
    }
}
