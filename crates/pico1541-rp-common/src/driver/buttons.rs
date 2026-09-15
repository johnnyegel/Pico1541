use embassy_rp::gpio::{Input, Pin, Pull};
use embassy_rp::Peri;
use pico1541_manager::InputDriver;

pub struct DirectKeyboardInput {
    buttons: [Input<'static>; 4],
}

impl DirectKeyboardInput {
    pub fn new(
        btn0: Peri<'static, impl Pin>,
        btn1: Peri<'static, impl Pin>,
        btn2: Peri<'static, impl Pin>,
        btn3: Peri<'static, impl Pin>,
    ) -> Self {
        Self {
            buttons: [
                Input::new(btn0, Pull::Up),
                Input::new(btn1, Pull::Up),
                Input::new(btn2, Pull::Up),
                Input::new(btn3, Pull::Up),
            ],
        }
    }
}

impl InputDriver for DirectKeyboardInput {
    async fn wait_for_change(&mut self) {
        let (btn0_slice, rest) = self.buttons.split_at_mut(1);
        let (btn1_slice, rest) = rest.split_at_mut(1);
        let (btn2_slice, btn3_slice) = rest.split_at_mut(1);

        let _ = embassy_futures::select::select4(
            btn0_slice[0].wait_for_any_edge(),
            btn1_slice[0].wait_for_any_edge(),
            btn2_slice[0].wait_for_any_edge(),
            btn3_slice[0].wait_for_any_edge(),
        )
        .await;
    }

    async fn scan(&mut self) -> u8 {
        let mut pressed = 0;

        for (i, btn) in self.buttons.iter().enumerate() {
            if btn.is_low() {
                pressed |= 1 << i;
            }
        }
        pressed
    }
}
