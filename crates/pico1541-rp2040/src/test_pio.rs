fn main() {
    let prg = pio::pio_asm!(
        ".side_set 2 opt",
        "nop",
        "nop [1]",
        "nop side 0",
        "nop side 1",
        "nop side 2",
        "nop side 3"
    );
    for code in prg.program.code {
        println!("{:016b}", code);
    }
}
