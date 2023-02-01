EODMS Data Downloader CLI
=========================

## Overview

The **eodms_data_downloader** is used to download directories and files from the EODMS Data Distribution System via the HTTPS interface.

## Building from source

### Rust setup

See https://www.rust-lang.org/learn/get-started

### Building the tool

1. Clone the repository:
	
	```dos
	git clone https://github.com/eodms-sgdot/eodms-data-downloader.git
	```
	
2. Install the tool:

	```dos
	cd eodms-data-downloader
	cargo build --path .
	```
  
## Install Binary (if not using source)

Download and install the appropriate binary for your architecture from:

https://github.com/eodms-sgdot/eodms-data-downloader/releases
	
## Configuration

Configuration for the utility can be found in the **config.ini** file in the home folder under ".eodms".

In the config file, you can: 

- Store credentials for authenticated access to the data server

For more in-depth information on the configuration file, see [Config File](https://github.com/eodms-sgdot/eodms-cli/wiki/Config-File#section-credentials).

## Command-line options

--url and --outdir options are mandatory
--rec will recursively download a directory
--include can be used to set an inclusion regular expression run against file names (not directories)
--threads sets the number of download threads (default is 4)
--stripdir will strip the directories before the last directory in the path in the output directory (--outdir)

```
Usage: eodms-data-downloader [OPTIONS] --url <url> --outdir <out>

Options:
  -u, --url <url>            
  -o, --outdir <out>         output directory
  -r, --rec                  recursive
  -i, --include <inc>        include regex
  -t, --threads <threads>    number of download threads
  -l, --loglevel <loglevel>  logging level off, error, info, debug, trace
  -s, --stripdirs            strip the leading directories
  -h, --help                 Print help
  -V, --version              Print version
```

## Contact

If you have any questions or require support, please contact the EODMS Support Team at eodms-sgdot@nrcan-rncan.gc.ca.

## License

MIT License

Copyright (c) His Majesty the King in Right of Canada, as 
represented by the Minister of Natural Resources, 2023

Permission is hereby granted, free of charge, to any person obtaining a 
copy of this software and associated documentation files (the "Software"), 
to deal in the Software without restriction, including without limitation 
the rights to use, copy, modify, merge, publish, distribute, sublicense, 
and/or sell copies of the Software, and to permit persons to whom the 
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in 
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING 
FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER 
DEALINGS IN THE SOFTWARE.
