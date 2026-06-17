#include <stdio.h>

int main() {
    int sectors = 21;
    int interleave = 10;
    
    for (int i = 0; i < sectors; i++) {
        printf("Sector %d -> %d\n", i, (i * interleave) % sectors);
    }
    return 0;
}
