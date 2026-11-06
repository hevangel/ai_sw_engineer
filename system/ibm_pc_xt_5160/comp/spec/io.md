# COMP: I/O contract

Every call is INT 21h unless named otherwise; the rewrite makes the same calls in the same order (spec D1-D5 are the differences).

| Channel | DOS call | Use |
|---|---|---|
| DOS version | AH=30h | AX must be 0002h; else the version message (09h) and INT 20h |
| Drives of the parameters | AX at entry | `(AH \| AL) == FFh`: `Invalid drive specification`, INT 20h |
| Switch character | AX=3700h | DL other than `/`: paths use `/` |
| Memory | PSP:0002h | the large buffer, 4 KiB to 60 KiB in multiples of 4 KiB; under 4 KiB free: `Insufficient memory`, INT 20h |
| Default drive | AH=19h | the drive put in front of a name without one |
| Command tail | PSP:0080h, 0081h | the two names; the first also as DOS parsed it into the FCB at PSP:005Ch |
| Name parse | AX=2901h, 2902h | the second name's FCB; a typed name's FCB; the file part after a directory |
| Directories | AH=47h, 3Bh | saves each drive's current directory, makes a name's directory current, changes back |
| Prompts | AH=0Ah | a name, 127 characters at most (echoed, CR without LF) |
| Files | AH=1Ah, 11h, 12h | search first/next for the first name (normal files) |
| | AH=0Fh | opens both files (FCBs; never closed, nothing is written) |
| | AH=27h, record size 1 | random block reads: the large buffer (DTA in its own segment) and 4 KiB pieces |
| Console | AH=09h, 02h | messages (`$`-terminated) and the offset of a difference character by character; BEL (07h) ends the error messages |
| Compare more | AX=0C01h | a key with echo after flushing type-ahead: Y/y again, N/n ends, others ask again |
| Exit | INT 20h | no exit code (ERRORLEVEL is not set) |
