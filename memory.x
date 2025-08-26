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

