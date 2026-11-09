; PRINT's resident part and the start of its transient part (spec D1-D3), in place of the shared
; runtime/crt0_com.s. Linked first (.text.start) at 100h by link_com.ld, so every symbol is the
; offset the specification gives it; each `.org` stops the build if what is before it grows past
; the original's offset.
;
; 100h-4FBh  the jump to the start, the resident stack (SP = 183h), the variables, the messages
;            for the list device and the record buffer, at the original's offsets (spec Layout)
; 4FCh-529h  the rewrite's own variables: the byte written, the request header, the driver
;            call pointer, the saved INT 13h/17h/14h/5 vectors, the printer and port numbers
;            (lib.rs: BYTE, REQUEST, CALL, OLD13, OLD17, OLD14, OLD5, PRINTER, PORT; spec D2)
; 52Bh-A65h  the handlers: INT 1Ch, 28h, 2Fh, 24h, 13h, 17h, 14h, 5, the print slice, the read,
;            the device routines (spec "The resident part"; the same logic is lib.rs `resident`)
; A66h-BF9h  the queue: 10 entries of 28h bytes in a ring (spec Layout)
; BFAh       print_start: the transient part's start (SP, .bss, main); INT 27h keeps 0-BF9h
;
; DS = CS inside the slice, the INT 2Fh, INT 24h handlers and the device routines; the hooks that
; run with the caller's DS address the resident data with CS:.

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	.byte	0xe9				; 100h: JMP print_start
	.word	print_start - (_start + 3)

	.org	0x183 - 0x100			; 103h-182h: the slice's stack
	.globl	print_indos
print_indos:
	.word	0, 0				; 183h: DOS's InDOS flag (offset, segment)
	.globl	print_old1c
print_old1c:
	.word	0, 0				; 187h
print_busy:
	.byte	0				; 18Bh: printing, or a BIOS disk call is running
print_idle:
	.byte	0				; 18Ch: the slice was started by INT 28h
print_ticks:
	.byte	0				; 18Dh: ticks since the slice started
print_since:
	.byte	0				; 18Eh: ticks since the last byte
print_wait:
	.byte	1				; 18Fh: ticks before the next timer slice
print_serving:
	.byte	0				; 190h: an INT 2Fh request is being served
	.globl	print_old28
print_old28:
	.word	0, 0				; 191h
print_shown:
	.byte	0				; 195h: `Canceled by operator` printed in this request
print_ss:
	.word	0				; 196h
print_sp:
	.word	0				; 198h
print_dta:
	.word	0, 0				; 19Ah
print_old24:
	.word	0, 0				; 19Eh
	.globl	print_device
print_device:
	.word	0, 0				; 1A2h: the driver's header (offset, segment)
print_column:
	.byte	0				; 1A6h
print_next:
	.word	0x4fc				; 1A7h: the next byte of the buffer; 4FCh: empty
print_head:
	.word	0xa6a				; 1A9h: the entry being printed
print_last:
	.word	0				; 1ABh
print_prev:
	.word	0				; 1ADh
print_error:
	.byte	0				; 1AFh: a critical error during the read

; The messages for the list device (spec "INT 24h", "INT 2Fh").
msg_banner:					; 1B0h
	.ascii	"\r\n\r\n**********\r\n$"
msg_reading:					; 1C1h
	.ascii	" error reading file\r\n"
msg_name:					; 1D6h: the file's name, filled in
	.ascii	" :        .   "
msg_end:					; 1E4h
	.byte	0x0d, 0x0c, 0x07, 0x24
msg_canceled:					; 1E8h
	.ascii	"\r\n\r\n"
msg_canceled_name:				; 1ECh
	.ascii	" :        .    Canceled by operator$"
msg_all:					; 210h
	.ascii	"\r\n\r\nAll files canceled by operator$"
msg_table:					; 233h: by error code 0-0Ch
	.word	msg_e0, msg_e1, msg_e2, msg_e3, msg_e4, msg_e5, msg_e6
	.word	msg_e7, msg_e8, msg_e9, msg_e10, msg_e11, msg_e12
msg_e0:	.ascii	"Write protect$"
msg_e1:	.ascii	"Bad unit$"
msg_e2:	.ascii	"Not ready$"
msg_e3:	.ascii	"Bad command$"
msg_e4:	.ascii	"Data$"
msg_e5:	.ascii	"Bad call format$"
msg_e6:	.ascii	"Seek$"
msg_e7:	.ascii	"Non-DOS disk$"
msg_e8:	.ascii	"Sector not found$"
msg_e9:	.ascii	"No paper$"
msg_e10: .ascii	"Write fault$"
msg_e11: .ascii	"Read fault$"
msg_e12: .ascii	"Disk$"
msg_fat:					; 2D7h
	.ascii	"File allocation table bad drive "
msg_fat_drive:					; 2F7h: AL is added to it (never put back)
	.ascii	"A.\r\n$"

	.org	0x2fc - 0x100
print_buffer:					; 2FCh-4FBh: the record buffer, the reads' DTA
	.zero	512

; The rewrite's own variables (spec D2).
	.org	0x4fc - 0x100
print_byte:
	.byte	0				; 4FCh: the byte written
print_request:					; 4FDh: the driver's request header, 22 bytes
	.byte	0x16				; length
	.byte	0				; unit
print_req_cmd:
	.byte	0				; 4FFh: 8 write, 0Ah output status
print_req_status:
	.word	0				; 500h
	.zero	8				; 502h: reserved
	.byte	0				; 50Ah: media
	.word	print_byte			; 50Bh: the transfer address (offset)
	.globl	print_req_segment
print_req_segment:
	.word	0				; 50Dh: its segment (CS, set by the install)
print_req_count:
	.word	1				; 50Fh
	.word	0				; 511h: start sector
	.globl	print_call
print_call:
	.word	0, 0				; 513h: the driver routine called (offset, segment)
	.globl	print_old13
print_old13:
	.word	0, 0				; 517h
	.globl	print_old17
print_old17:
	.word	0, 0				; 51Bh
	.globl	print_old14
print_old14:
	.word	0, 0				; 51Fh
	.globl	print_old5
print_old5:
	.word	0, 0				; 523h
	.globl	print_printer
print_printer:
	.word	0				; 527h: the printer number, 0-2 (INT 17h DX)
	.globl	print_port
print_port:
	.word	0				; 529h: the serial port number, 0-1 (INT 14h DX)

	.org	0x52b - 0x100
; INT 1Ch: count the tick; every ninth tick outside DOS, a print slice.
	.globl	print_int1c
print_int1c:
	inc	byte ptr cs:[print_ticks]
	inc	byte ptr cs:[print_since]
	cmp	byte ptr cs:[print_wait], 0
	je	.L1c_due
	dec	byte ptr cs:[print_wait]
	jmp	.L1c_chain
.L1c_due:
	cmp	byte ptr cs:[print_busy], 0
	jne	.L1c_chain
	push	ds
	push	si
	lds	si, dword ptr cs:[print_indos]
	cmp	byte ptr [si], 0
	pop	si
	pop	ds
	jne	.L1c_chain
	inc	byte ptr cs:[print_busy]
	mov	byte ptr cs:[print_ticks], 0
	mov	byte ptr cs:[print_since], 0
	sti
	push	ax
	mov	al, 0x20			; EOI: the next ticks are counted during the slice
	out	0x20, al
	pop	ax
	call	print_slice
	cli
	mov	byte ptr cs:[print_wait], 8
	mov	byte ptr cs:[print_busy], 0
.L1c_chain:
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[print_old1c]
	.word	print_old1c

; INT 28h (DOS is waiting for a key): a print slice of one byte.
	.globl	print_int28
print_int28:
	cmp	byte ptr cs:[print_busy], 0
	jne	.L28_chain
	inc	byte ptr cs:[print_busy]
	inc	byte ptr cs:[print_idle]
	sti
	call	print_slice
	cli
	mov	byte ptr cs:[print_idle], 0
	mov	byte ptr cs:[print_wait], 8
	mov	byte ptr cs:[print_busy], 0
.L28_chain:
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[print_old28]
	.word	print_old28

; The print slice: on PRINT's own stack, DS = CS; every register kept.
print_slice:
	push	si
	mov	si, word ptr cs:[print_head]
	cmp	byte ptr cs:[si + 2], 0xff
	pop	si
	jne	.Lslice_go
	ret
.Lslice_go:
	push	ax
	mov	word ptr cs:[print_ss], ss
	mov	word ptr cs:[print_sp], sp
	mov	ax, cs
	cli
	mov	ss, ax
	mov	sp, 0x183
	sti
	push	es
	push	ds
	push	bx
	push	cx
	push	dx
	push	si
	push	di
	push	cs
	pop	ds
	cmp	word ptr [print_next], 0x4fc
	jne	.Lslice_loop
	call	print_read
	jmp	.Lslice_end
.Lslice_loop:
	mov	bx, word ptr [print_next]
	cmp	bx, 0x4fc
	je	.Lslice_end			; the buffer is exhausted: the next slice reads
	cmp	byte ptr [print_idle], 0
	jne	.Lslice_ask
	cmp	byte ptr [print_ticks], 2
	jae	.Lslice_end			; a timer slice has had two ticks
.Lslice_ask:
	call	print_status
	je	.Lslice_byte
	cmp	byte ptr [print_idle], 0
	jne	.Lslice_end
	cmp	byte ptr [print_since], 1
	jae	.Lslice_end
	jmp	.Lslice_loop
.Lslice_byte:
	mov	al, byte ptr [bx]
	cmp	al, 0x1a
	jne	.Lslice_not_eof
	mov	al, 0x0c			; ^Z: the end of the file
	call	print_put
	call	print_next_file
	jmp	.Lslice_end
.Lslice_not_eof:
	cmp	al, 0x0d
	jne	.Lslice_not_cr
	mov	byte ptr [print_column], 0
.Lslice_not_cr:
	cmp	al, 0x09
	jne	.Lslice_not_tab
	mov	cl, byte ptr [print_column]	; spaces to the next multiple of 8
	or	cl, 0xf8
	neg	cl
	xor	ch, ch
.Lslice_tab:
	mov	al, 0x20
	inc	byte ptr [print_column]
	push	cx
	call	print_write
	pop	cx
	loop	.Lslice_tab
	jmp	.Lslice_sent
.Lslice_not_tab:
	cmp	al, 0x08
	jne	.Lslice_not_bs
	dec	byte ptr [print_column]
.Lslice_not_bs:
	cmp	al, 0x20
	jb	.Lslice_write
	inc	byte ptr [print_column]
.Lslice_write:
	call	print_write
.Lslice_sent:
	inc	word ptr [print_next]
	mov	byte ptr [print_since], 0
	cmp	byte ptr [print_idle], 0
	je	.Lslice_loop			; an idle slice sends one byte
.Lslice_end:
	pop	di
	pop	si
	pop	dx
	pop	cx
	pop	bx
	pop	ds
	pop	es
	cli
	mov	ss, word ptr cs:[print_ss]
	mov	sp, word ptr cs:[print_sp]
	sti
	pop	ax
	ret

; The read: the head entry's next 512-byte record into the buffer (function 14h), with this
; INT 24h handler and the buffer as the DTA for the call. DS = CS.
print_read:
	mov	ax, 0x3524
	int	0x21
	mov	word ptr [print_old24 + 2], es
	mov	word ptr [print_old24], bx
	mov	dx, print_int24
	mov	ax, 0x2524
	int	0x21
	mov	ah, 0x2f
	int	0x21
	mov	word ptr [print_dta + 2], es
	mov	word ptr [print_dta], bx
	mov	dx, print_buffer
	mov	ah, 0x1a
	int	0x21
	mov	byte ptr [print_error], 0
	mov	dx, word ptr [print_head]
	add	dx, 2
	mov	ah, 0x14
	int	0x21
	push	ax
	push	ds
	lds	dx, dword ptr cs:[print_dta]
	mov	ah, 0x1a
	int	0x21
	lds	dx, dword ptr cs:[print_old24]
	mov	ax, 0x2524
	int	0x21
	pop	ds
	pop	ax
	cmp	byte ptr [print_error], 0
	jne	print_next_file			; a critical error: the next file
	cmp	al, 1
	je	.Lread_eof
	mov	word ptr [print_next], print_buffer
	ret
.Lread_eof:
	mov	al, 0x0c			; no data: the end of the file
	call	print_put
; The next file: the head entry freed, the head moved on, the buffer empty, the column 0.
print_next_file:
	mov	si, word ptr [print_head]
	mov	byte ptr [si + 2], 0xff
	mov	si, word ptr [si]
	mov	word ptr [print_head], si
	mov	word ptr [print_next], 0x4fc
	mov	byte ptr [print_column], 0
	ret

; INT 24h during the read: the error on the list device, once; AL = 0 (ignore).
print_int24:
	sti
	cmp	byte ptr cs:[print_error], 0
	jne	.L24_done
	push	bx
	push	cx
	push	dx
	push	di
	push	si
	push	bp
	push	es
	push	ds
	push	cs
	pop	ds
	push	cs
	pop	es
	cld
	add	byte ptr [msg_fat_drive], al
	mov	si, msg_banner
	call	print_string
	test	ah, 0x80
	jne	.L24_fat
	and	di, 0xff
	cmp	di, 0x0c
	jbe	.L24_code
	mov	di, 0x0c
.L24_code:
	shl	di, 1
	mov	si, word ptr [di + msg_table]
	call	print_string
	mov	di, msg_name
	mov	si, word ptr [print_head]
	add	si, 2
	call	print_name
	mov	si, msg_reading
	call	print_string
.L24_set:
	inc	byte ptr [print_error]
	pop	ds
	pop	es
	pop	bp
	pop	si
	pop	di
	pop	dx
	pop	cx
	pop	bx
.L24_done:
	xor	al, al
	iret
.L24_fat:
	mov	si, msg_fat
	call	print_string
	jmp	.L24_set

; The FCB at DS:SI into the message at ES:DI: its drive byte + 40h, `:`, the name, `.`, the
; extension.
print_name:
	lodsb
	add	al, 0x40
	stosb
	inc	di
	mov	cx, 4
	rep
	movsw
	inc	di
	movsw
	movsb
	ret

; INT 2Fh: the transient part's requests. AH = 0 submit the FCB at DS:DX, 1 cancel (DX = FFFFh:
; every file), other: status. Returns AL (1: the queue is full), AH = the entries in use,
; ES:BX = the queue, DX = the head. AX = FFFFh while a request is being served.
print_reply_busy:
	mov	ax, 0xffff
	iret
	.globl	print_int2f
print_int2f:
	cmp	byte ptr cs:[print_serving], 0
	jne	print_reply_busy
	inc	byte ptr cs:[print_serving]
	sti
	push	si
	push	di
	push	cx
	push	ds				; the caller's DS: the last push (submit, cancel)
	push	cs
	pop	ds
	cld
	mov	byte ptr [print_shown], 0
	or	ah, ah
	je	.L2f_submit
	cmp	ah, 1
	je	.L2f_cancel
	xor	al, al
.L2f_reply:					; AL = the result
	push	ax
	xor	ah, ah
	mov	si, print_queue
	mov	cx, word ptr [print_queue_len]
.L2f_count:
	cmp	byte ptr [si + 2], 0xff
	je	.L2f_free
	inc	ah
.L2f_free:
	add	si, 0x28
	loop	.L2f_count
	mov	bx, print_queue
	mov	dx, word ptr [print_head]
	push	ds
	pop	es
	mov	ch, ah
	pop	ax
	mov	ah, ch
.L2f_wait:
	cmp	byte ptr [print_busy], 0
	jne	.L2f_wait
	inc	byte ptr [print_busy]
	call	print_status
	mov	byte ptr [print_busy], 0
	pop	ds
	pop	cx
	pop	di
	pop	si
	mov	byte ptr cs:[print_serving], 0
	iret

.L2f_submit:
	mov	si, word ptr [print_head]
	mov	cx, word ptr [print_queue_len]
.L2f_wait_submit:
	cmp	byte ptr [print_busy], 0
	jne	.L2f_wait_submit
	inc	byte ptr [print_busy]
.L2f_find:
	cmp	byte ptr [si + 2], 0xff
	je	.L2f_found
	mov	si, word ptr [si]
	loop	.L2f_find
	mov	byte ptr [print_busy], 0
	mov	al, 1				; the queue is full
	jmp	.L2f_reply
.L2f_found:
	push	ds
	pop	es
	pop	ds				; the caller's DS
	push	ds
	push	si
	mov	di, si
	add	di, 2
	mov	si, dx
	mov	cx, 0x13			; the 38 bytes of the opened FCB
	rep
	movsw
	pop	si
	push	es
	pop	ds
	mov	word ptr [si + 0x0e], 0		; current block
	mov	byte ptr [si + 0x22], 0		; current record
	mov	word ptr [si + 0x10], 0x200	; record size
	mov	byte ptr [print_busy], 0
	xor	al, al
	jmp	.L2f_reply

.L2f_cancel:
	mov	cx, word ptr [print_queue_len]
.L2f_wait_cancel:
	cmp	byte ptr [print_busy], 0
	jne	.L2f_wait_cancel
	inc	byte ptr [print_busy]
	mov	si, word ptr [print_head]
	cmp	dx, 0xffff
	je	.L2f_all
	mov	bx, word ptr [si]		; the last entry: the one before the head
.L2f_last:
	cmp	word ptr [bx], si
	je	.L2f_got_last
	mov	bx, word ptr [bx]
	jmp	.L2f_last
.L2f_got_last:
	mov	word ptr [print_last], bx
	mov	word ptr [print_prev], bx
	pop	es				; the caller's DS
	push	es
.L2f_walk:					; SI = the entry, CX = entries left
	mov	bx, si
	mov	di, dx
	add	si, 2
	cmp	byte ptr [si], 0xff
	je	.L2f_cancel_done		; a free entry: the end of the files in use
	cmpsb					; the drive bytes
	jne	.L2f_skip
	push	cx
	mov	cx, 11
.L2f_char:
	mov	al, byte ptr es:[di]
	inc	di
	call	print_upper
	mov	ah, al
	lodsb
	call	print_upper
	cmp	ah, 0x3f			; `?` matches anything
	je	.L2f_char_ok
	cmp	ah, al
	jne	.L2f_mismatch
.L2f_char_ok:
	loop	.L2f_char
	pop	cx
	mov	ah, 0xff
	xchg	byte ptr [bx + 2], ah		; freed; AH = its drive byte
	cmp	bx, word ptr [print_head]
	jne	.L2f_move
	mov	al, 1
	xchg	byte ptr [print_shown], al
	or	al, al
	jne	.L2f_head_next
	push	es
	push	cs
	pop	es
	mov	di, msg_canceled_name
	mov	si, bx
	add	si, 3
	mov	al, ah
	add	al, 0x40
	stosb
	inc	di
	mov	cx, 4
	rep
	movsw
	inc	di
	movsw
	movsb
	pop	es
	mov	si, msg_canceled
	call	print_string
	mov	si, msg_end
	call	print_string
.L2f_head_next:
	push	cx
	call	print_next_file
.L2f_mismatch:
	pop	cx
.L2f_skip:
	mov	word ptr [print_prev], bx
	mov	bx, word ptr [bx]
.L2f_step:
	mov	si, bx
	loop	.L2f_walk
.L2f_cancel_done:
	jmp	.L2f_cancel_exit
.L2f_move:					; behind the last entry
	mov	ax, word ptr [bx]
	cmp	ax, word ptr [print_head]
	je	.L2f_skip
	mov	si, word ptr [print_prev]
	mov	word ptr [si], ax
	mov	si, word ptr [print_head]
	mov	word ptr [bx], si
	mov	si, word ptr [print_last]
	mov	word ptr [si], bx
	mov	word ptr [print_last], bx
	mov	bx, ax
	jmp	.L2f_step
.L2f_all:
	cmp	byte ptr [si + 2], 0xff		; ZF: the head was free (kept by MOV and LOOP)
.L2f_all_free:
	mov	byte ptr [si + 2], 0xff
	mov	si, word ptr [si]
	loop	.L2f_all_free
	je	.L2f_all_quiet
	mov	si, msg_all
	call	print_string
	mov	si, msg_end
	call	print_string
.L2f_all_quiet:
	mov	word ptr [print_next], 0x4fc
.L2f_cancel_exit:
	mov	byte ptr [print_busy], 0
	xor	ax, ax
	jmp	.L2f_reply

; AL made capital if it is a small letter.
print_upper:
	cmp	al, 0x61
	jb	.Lupper_done
	cmp	al, 0x7a
	ja	.Lupper_done
	sub	al, 0x20
.Lupper_done:
	ret

; The `$`-terminated string at DS:SI to the list device (print_put).
print_string:
	lodsb
	cmp	al, 0x24
	je	.Lstring_done
	call	print_put
	jmp	print_string
.Lstring_done:
	ret

; AL to the list device once it is ready; dropped once the device has not been ready for more
; than 20000 status calls.
print_put:
	push	bx
.Lput_ask:
	call	print_status
	je	.Lput_write
	cmp	word ptr [print_errors], 20000
	ja	.Lput_done
	jmp	.Lput_ask
.Lput_write:
	call	print_write
.Lput_done:
	pop	bx
	ret

; The device's output status: ZF = 1 when it is ready. Counts the calls since it was last
; ready (A68h). DS = CS.
print_status:
	push	bx
	inc	word ptr [print_errors]
	mov	bl, 0x0a
	call	print_driver
	test	word ptr [print_req_status], 0x8000
	je	.Lstatus_no_error
	or	word ptr [print_req_status], 0x200	; an error is busy
.Lstatus_no_error:
	test	word ptr [print_req_status], 0x200
	jne	.Lstatus_done
	mov	word ptr [print_errors], 0
.Lstatus_done:
	pop	bx
	ret

; AL to the device (command 8, one byte). DS = CS.
print_write:
	push	bx
	mov	byte ptr [print_byte], al
	mov	bl, 8
	call	print_driver
	pop	bx
	ret

; The request (command BL) to the device driver: its strategy routine, then its interrupt
; routine, with ES:BX = the request header.
print_driver:
	push	es
	mov	byte ptr [print_req_cmd], bl
	mov	bx, cs
	mov	es, bx
	mov	bx, print_request
	mov	word ptr [print_req_status], 0
	mov	word ptr [print_req_count], 1
	push	ds
	push	si
	push	ax
	lds	si, dword ptr cs:[print_device]
	mov	ax, word ptr [si + 6]
	mov	word ptr cs:[print_call], ax
	.byte	0x2e, 0xff, 0x1e		; CALL FAR CS:[print_call]
	.word	print_call
	mov	ax, word ptr [si + 8]
	mov	word ptr cs:[print_call], ax
	.byte	0x2e, 0xff, 0x1e		; CALL FAR CS:[print_call]
	.word	print_call
	pop	ax
	pop	si
	pop	ds
	pop	es
	ret

; INT 13h: busy while the BIOS disk call runs; the BIOS's flags returned.
	.globl	print_int13
print_int13:
	pushf
	inc	byte ptr cs:[print_busy]
	.byte	0x2e, 0xff, 0x1e		; CALL FAR CS:[print_old13], with the flags pushed
	.word	print_old13
	pushf
	dec	byte ptr cs:[print_busy]
	popf
	retf	2

; INT 17h: another program's call for PRINT's printer fails while a file is queued.
	.globl	print_int17
print_int17:
	push	si
	mov	si, word ptr cs:[print_head]
	cmp	byte ptr cs:[si + 2], 0xff
	pop	si
	je	.L17_chain
	cmp	dx, word ptr cs:[print_printer]
	jne	.L17_chain
	cmp	byte ptr cs:[print_busy], 0
	jne	.L17_chain
	sti
	mov	ah, 0xa1
	iret
.L17_chain:
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[print_old17]
	.word	print_old17

; INT 14h: likewise for PRINT's serial port: a time-out.
	.globl	print_int14
print_int14:
	push	si
	mov	si, word ptr cs:[print_head]
	cmp	byte ptr cs:[si + 2], 0xff
	pop	si
	je	.L14_chain
	cmp	dx, word ptr cs:[print_port]
	jne	.L14_chain
	cmp	byte ptr cs:[print_busy], 0
	jne	.L14_chain
	sti
	or	ah, ah
	je	.L14_zero
	cmp	ah, 2
	jbe	.L14_keep
.L14_zero:
	mov	al, 0
.L14_keep:
	mov	ah, 0x80
	iret
.L14_chain:
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[print_old14]
	.word	print_old14

; INT 5 (Print Screen): ignored while a file is queued for the first printer.
	.globl	print_int5
print_int5:
	push	si
	mov	si, word ptr cs:[print_head]
	cmp	byte ptr cs:[si + 2], 0xff
	pop	si
	je	.L5_chain
	cmp	word ptr cs:[print_printer], 0
	jne	.L5_chain
	iret
.L5_chain:
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[print_old5]
	.word	print_old5

	.org	0xa66 - 0x100
print_queue_len:
	.word	10				; A66h
	.globl	print_errors
print_errors:
	.word	0				; A68h: status calls since the device was ready
print_queue:					; A6Ah: the ring, every entry free
	.word	print_queue + 0x28
	.fill	38, 1, 0xff
	.word	print_queue + 0x50
	.fill	38, 1, 0xff
	.word	print_queue + 0x78
	.fill	38, 1, 0xff
	.word	print_queue + 0xa0
	.fill	38, 1, 0xff
	.word	print_queue + 0xc8
	.fill	38, 1, 0xff
	.word	print_queue + 0xf0
	.fill	38, 1, 0xff
	.word	print_queue + 0x118
	.fill	38, 1, 0xff
	.word	print_queue + 0x140
	.fill	38, 1, 0xff
	.word	print_queue + 0x168
	.fill	38, 1, 0xff
	.word	print_queue
	.fill	38, 1, 0xff

; The transient part. DOS jumped to 100h with CS = DS = ES = SS = the PSP's segment. As
; crt0_com.s: SP at the end of the program's memory (at most 0FFF0h), .bss cleared, main called,
; INT 20h.
	.org	0xbfa - 0x100
print_start:					; BFAh
	cld
	mov	ax, word ptr [2]
	mov	dx, cs
	sub	ax, dx
	cmp	ax, 0x0fff
	jb	.Lsized
	mov	ax, 0x0fff
.Lsized:
	mov	cl, 4
	shl	ax, cl
	mov	sp, ax
	mov	di, __bss_start
	mov	cx, __bss_end
	sub	cx, di
	xor	al, al
	rep
	stosb
	call	main
	int	0x20

; void print_int2f_call(struct Regs *r): as dos_call (runtime/dos.s), INT 2Fh.
	.section	.text.print_int2f_call,"ax",@progbits
	.globl	print_int2f_call
print_int2f_call:
	push	bp
	push	bx
	push	si
	push	di
	push	ds
	push	es
	mov	bp, ax
	push	bp
	mov	cx, word ptr [bp + 4]
	mov	dx, word ptr [bp + 6]
	mov	si, word ptr [bp + 8]
	mov	di, word ptr [bp + 10]
	mov	es, word ptr [bp + 12]
	mov	bx, word ptr [bp + 2]
	mov	ax, word ptr [bp + 14]
	mov	ds, ax
	mov	ax, word ptr [bp]
	int	0x2f
	pushf
	push	ax
	mov	bp, sp
	mov	bp, word ptr [bp + 4]
	mov	word ptr [bp + 2], bx
	mov	word ptr [bp + 4], cx
	mov	word ptr [bp + 6], dx
	mov	word ptr [bp + 8], si
	mov	word ptr [bp + 10], di
	mov	word ptr [bp + 12], es
	mov	word ptr [bp + 14], ds
	pop	ax
	mov	word ptr [bp], ax
	pop	ax
	mov	word ptr [bp + 16], ax
	add	sp, 2
	pop	es
	pop	ds
	pop	di
	pop	si
	pop	bx
	pop	bp
	ret

; void print_keep(u16 bytes) -> !: INT 27h with DX = bytes (CS is the PSP's segment).
	.section	.text.print_keep,"ax",@progbits
	.globl	print_keep
print_keep:
	mov	dx, ax
	int	0x27
