# Confidential Battleship Game

A privacy-preserving battleship game built on Solana using Arcium's encrypted supercomputer. This implementation ensures that ship locations remain completely hidden from opponents and even from on-chain observers through the use of encrypted computations.

## 🚨 Important Notice

**This project is for educational purposes and testing only. It has not been audited and should not be used in production environments.**

## 🎮 Game Overview

This battleship game leverages Arcium's encrypted computation capabilities to create a fully confidential gaming experience where:

- Ship placements are encrypted and never revealed on-chain
- Player moves are processed through encrypted instructions
- Game state transitions happen within arcium mxe
- Only the necessary information (hit/miss results) is revealed to players

## 🔧 Technology Stack

- **Solana**: Blockchain platform for game state and transactions
- **Anchor**: Solana development framework
- **Arcium**: Encrypted supercomputer for confidential computations
- **Rust**: Primary programming language

## 🏗️ Architecture

The game consists of three main components:

### 1. Solana Program (`programs/confidential_battleship_game/`)
- Manages game initialization and player interactions
- Handles computation queuing and callbacks
- Maintains public game state (turn order, ships remaining)

### 2. Encrypted Instructions (`encrypted-ixs/`)
- `init_player_ship_fleet_location`: Initializes empty encrypted fleet state
- `place_ships`: Processes ship placement in encrypted environment
- `take_turn`: Handles guess processing and hit detection

### 3. Game Flow
1. **Game Creation**: Initialize game with two players
2. **Ship Placement**: Players secretly place their 17 ships
3. **Turn-Based Gameplay**: Players take turns guessing opponent locations
4. **Victory Condition**: First player to sink all opponent ships wins

## 🎯 Key Features

### Privacy-Preserving Gameplay
- Ship locations never exposed on-chain
- Encrypted computation ensures fair play

### Decentralized Architecture
- No trusted third party required
- All game logic runs on Solana
- Transparent and verifiable game rules

### Secure State Management
- Encrypted fleet state stored on-chain
- Nonce-based encryption for state updates
- Callback-driven state transitions

## 🚀 Getting Started

### Prerequisites
- Rust
- Solana CLI tools
- Anchor Framework
- Arcium CLI
- Node.js

### Installation

1. Clone the repository:
```bash
git clone https://github.com/Mahhheshh/confidential_battleship_game
cd confidential_battleship_game
```

2. Install dependencies:
```bash
# Install dependencies
arcium build
```

### Running Tests

Tests are currently in development. To run available tests:

```bash
arcium test
```

## 🎲 How to Play

### Game Setup
1. Player 1 initializes a new game with Player 2's public key
2. Both players place their ships on a 10x10 grid:
    - Carrier (5 spaces)
    - Battleship (4 spaces)
    - Cruiser (3 spaces)
    - Submarine (3 spaces)
    - Destroyer (2 spaces)
3. Game automatically starts with Player 1's turn

### Gameplay
1. Current player submits a guess (row, col coordinates)
2. Encrypted computation processes the guess against opponent's fleet
3. Result (hit/miss) is revealed, overall fleet health is updated if hit (total of 17 spaces across all ships)
4. Turn passes to the next player
5. Game ends when one player's fleet is completely destroyed

### Game States
- `PlacingShips`: Initial state, players placing ships
- `Player1Turn`: Player 1's turn to make a guess
- `Player2Turn`: Player 2's turn to make a guess
- `Finished`: Game completed, winner determined

## 📊 Game Data Structure

```rust
pub struct GameData {
    pub player_1: Pubkey,                    // Player 1 public key
    pub player_2: Pubkey,                    // Player 2 public key
    pub player_1_arcium_pubkey: [u8; 32],    // Encryption key for Player 1
    pub player_2_arcium_pubkey: [u8; 32],    // Encryption key for Player 2
    pub game_state: GameState,               // Current game state
    pub player_1_ships_left: u8,             // Ships remaining for Player 1
    pub player_2_ships_left: u8,             // Ships remaining for Player 2
    pub fleet_state_nonce: [u8; 16],         // Encryption nonce
    pub encrypted_fleet_state: [[u8; 32]; 34], // Encrypted ship's positions
}
```

## ⚠️ Limitations & Disclaimers

- **Educational Purpose Only**: This project is designed for learning and experimentation
- **No Security Audit**: Code has not undergone professional security review
- **Testing Phase**: Comprehensive tests are still being developed
- **Experimental Technology**: Arcium's encrypted computation is cutting-edge technology


## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **Arcium Team**: For providing the encrypted supercomputer infrastructure
- **Solana Labs**: For the robust blockchain platform
- **Anchor**: For the excellent development framework

**Remember: This is experimental software for educational purposes. Do not use in production environments without proper security audits.**