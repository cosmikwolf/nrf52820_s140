/* Linker script for the nRF52820 with SoftDevice S140 v7.3.0 */
MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* nRF52820: 256K flash, 32K RAM total */
  /* S140 v7.3.0 uses approximately 152K flash and requires specific RAM layout */
  /* According to S140 spec for nRF52820 */
  FLASH : ORIGIN = 0x00027000, LENGTH = 256K - 156K
  RAM : ORIGIN = 0x20002800, LENGTH = 32K - 10K
}

/* This is where the call stack will be allocated. */
/* The stack is of the full descending type. */
/* You may want to use this variable to locate the call stack and static
   variables in different memory regions. Below is shown the default value */
/* _stack_start = ORIGIN(RAM) + LENGTH(RAM); */

/* You can use this symbol to customize the location of the .text section */
/* If omitted it will place after .vector_table */
/* _stext = ORIGIN(FLASH) + 0x400; */

/* Example of putting non-initialized variables into custom RAM locations: */
/* This will require `noinit` feature to be activated */
/* SECTIONS {
     .SOME_REGION (NOLOAD) : ALIGN(4) {
       *(.SOME_REGION .SOME_REGION.*);
       . = ALIGN(4);
     } > SOME_REGION
   } INSERT AFTER .bss;
*/