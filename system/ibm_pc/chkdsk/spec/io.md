# chkdsk I/O contract

CHKDSK is a DOS program: it never touches the hardware. Its whole machine interface is **INT 20h/21h** (PC DOS 1.00 function calls), the PSP and the two default FCBs. The portable core talks to a `Dos` trait; the platform's COM runtime implements it with `int 21h`, and the host test implements it over an in-memory disk image.

| Operation | Call | Used for |
|---|---|---|
| terminate | INT 20h | end of program |
| write string | 21h AH=09h | `$`-terminated messages |
| write char | 21h AH=02h | file names |
| current drive | 21h AH=19h | save the caller's drive |
| select drive | 21h AH=0Eh | returns the drive count |
| set DTA | 21h AH=1Ah | directory search buffer |
| search first / next | 21h AH=11h, 12h | read the directory |
| FAT information | 21h AH=1Bh | FAT in memory, sectors per cluster, clusters, sector size |
| open / close | 21h AH=0Fh, 10h | rewrite a directory entry |
| delete | 21h AH=13h | entries that cannot be read |
| set random record / random write | 21h AH=24h/28h | write back repaired entries |
| set vector | 21h AH=25h | critical-error handler (INT 24h) |
| reset disk | 21h AH=0Dh | flush before leaving |
| PSP | words at 02h, 5Ch, 5Dh | top of memory, drive of the first parameter |

The oracle transcripts record the screen text and the final disk image.
