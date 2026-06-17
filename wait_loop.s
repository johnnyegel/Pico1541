$EDDD:
        BIT $1800      ; Check ATN (PB7)
        BMI $EDDD      ; Wait while ATN is asserted!
$EDE3:
        LDA $1800
        CMP $1800
        BNE $EDE3
        AND #$04       ; Check DATA (PB2)
        BEQ $EDF5      ; If 0, branch to error?
