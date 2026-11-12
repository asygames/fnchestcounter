// Fără consolă în spatele aplicației pe Windows (în release).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    fn_chest_counter_lib::run()
}
