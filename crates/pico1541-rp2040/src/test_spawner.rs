use embassy_executor::Spawner;
pub fn get_spawner() -> Spawner {
    Spawner::for_current_executor()
}
