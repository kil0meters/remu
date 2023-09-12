.global _start

_start:
    addi x5, x4, 10

    addi x1, x0,   1000  /* x1  = 1000 0x3E8 */
    addi x2, x1,   2000  /* x2  = 3000 0xBB8 */
    addi x3, x2,  -1000  /* x3  = 2000 0x7D0 */
    addi x4, x3,  -2000  /* x4  = 0    0x000 */
    addi x5, x4,   1000  /* x5  = 1000 0x3E8 */

    addi  a0, x0, 0       # set return code to 0 (success) for exit (93) function
    addi    a7, x0, 93     # set ecall to exit (93) function
    ecall                  # Call linux to terminate the program


; .data
; variable:
; 	.word 0xdeadbeef
