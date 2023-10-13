.global _start

fib:
    # Argument n is in a0
    beqz a0, is_zero    # n = 0?
    addi t0, a0, -1     # Hack: If a0 == 1 then t0 == 0
    beqz t0, is_one     # n = 1?

    # n > 1, do this the hard way

    addi sp, sp, -16    # Make room for two 64-Bit words on stack
    sd a0, 0(sp)        # Save original n
    sd ra, 8(sp)        # Save return address

    addi a0, a0, -1     # Now n-1 in a0
    jal fib             # Calculate fibo(n-1)

    ld t0, 0(sp)        # Get original n from stack
    sd a0, 0(sp)        # Save fibo(n-1) to stack in same place
    addi a0, t0, -2     # Now n-2 in a0
    jal fib             # Calculate fibo(n-2)

    ld t0, 0(sp)        # Get result of fibo(n-1) from stack
    add a0, a0, t0      # add fibo(n-1) and fibo(n-2)

    ld ra, 8(sp)        # Get return address from stack
    addi sp, sp, 16     # clean up stack
is_zero:
is_one:
    ret

_start:
    li a0, 35
    jal fib

    # addi  a0, x0, 0      # set return code to 0 (success) for exit (93) function
    addi  a7, x0, 93     # set ecall to exit (93) function
    ecall                # Call linux to terminate the program
