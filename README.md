# pdecrypt

Decrypt all pdf files in a directory, using a password list

## Motivation

I live in Thailand. I receive 6-7 financial statement PDF files every month, with a password based on my date of birth. The problem is 1. I hate opening encrypted PDF files 2. Some files have different format, which I have to trial and error to open them every month.

So instead of just spending 3 minutes to check all of them, I spent my holiday night creating a tool to decrypt all the PDF files in a directory. :D

## Installation

This only support direct build from source at the moment.

```sh
git clone GITHUB_HERE
cd pdecrypt
cargo build --release
cp target/release/pdecrypt .
```

## Usage

To begin, run `pdecrypt init dd/mm/yyyy THAI-CITIZEN-NO`
to configure the password list based on your date of birth.

Then, you can run `pdecrypt decrypt -i /path/to/pdfs/dir`
to generate a new directory with decrypted pdf files.

## License

This project is licensed under the terms of the MIT license.
