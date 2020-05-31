# PATCHY

Patchy is a command line tool for creating and applying binary file patches.

## How it works

The general algorithm is similar to `rsync`. The tool operates on two files: local **base** (old) and **other** (new). The **other** file is split into equal-size blocks and a pair of hashes is computed for each block: weak 32-bit hash using a rolling checksum similar to `adler-32` and a strong 128-bit hash using `blake3`. The **base** file is then scanned one byte at a time, maintaining a rolling hash of the block-sized window. If rolling hash of the current window matches some block weak hash computed for **other** file earlier, then a strong hash is computed for this window and checked against strong block hashes of the **other** file. This process finds blocks in the **base** file that can be reused when patching it to produce the **other** file. Finally, a patch command list is generated that tells which blocks need to be copied from **base** and from **other** files (as source/target byte offsets and sizes). Blocks that are missing from **base** as well as copy commands are written into the patch file which is then compressed using `zstd`.

Once the patch is generated, it can be applied simply by executing the copy commands, reading data either from **base** file or from the patch itself and writing to the output file (which must be different from **base**, as in-place patching is not implemented).

Patchy performs basic verification of the patch during generation by applying the patch to the **base** file in memory and comparing its hash to the hash of new file. When patch is applied from file later, the **base** file and patched output file hashes are checked against what's stored in patch metadata.

## Usage

### **diff**

`patchy diff [OPTIONS] <BASE> <OTHER> [PATCH]`

Compute difference between files specified by `BASE` and `OTHER` and optionally produce `PATCH` file which can be used to transform `BASE` into `OTHER` later. 

If `PATCH` is not specified, then the patch is still generated and verified in memory, but not written to disk.

Options:

* `-b <block>`
    * Patch block search window as log2(bytes)
    * Expected range: [6..24]
    * Default: 11 (2048 bytes)
* `-l <level>`
    * Compression level
    * Expected range: [1..22]
    * Default: 15    

### **patch**

`patchy patch <BASE> <PATCH> [OUTPUT]`

Apply a patch that was previously produced using `diff` command on the file specified by `BASE`, optionally writing out the result into `OUTPUT`.

If `OUTPUT` is not specified, then patching process is still performed and verified in memory, but no output is written to disk.
