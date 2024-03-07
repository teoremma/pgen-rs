# pgen-rs

`pgen-rs` is a tool used to query and filter [.pgen
files](https://www.cog-genomics.org/plink/2.0/input#pgen). It's like
[bcftools](https://samtools.github.io/bcftools) for .pgen files.

## Example usage

Print the chromosome and position (separated by space) of all variants which
have `G` as their alternate allele.

``` shell
$ pgen-rs query data/basic1/basic1 -i 'ALT == "G"' -f 'CHROM + " " + POS'
```

Filter the file to retain only the sample with ID `NA20900` and variants which
have `G` as their alternate allele, producing a VCF named `basic1.pgen-rs.vcf`.

``` shell
$ pgen-rs filter data/basic1/basic1 --include-sam 'IID == "NA20900"' --include-var 'ALT == "G"'
```

## Installation

Go to [releases](https://github.com/teoremma/pgen-rs/releases), pick a release,
and then either paste the provided shell command or download the binaries and
put them in your path manually.

## Subcommands

### `query`

Queries the pgen, outputting to stdout. Similar to [`bcftools
query`](https://samtools.github.io/bcftools/bcftools.html#query).

All expressions have as variables the metadata being queried. For example, if
querying the variants, `CHROM ` and `ID `are variables which contain their respective
values. This applies both for the expressions in the fstring and query.

```
Usage: pgen-rs query [OPTIONS] --fstring <QUERY_FSTRING> <PFILE_PREFIX>

Arguments:
  <PFILE_PREFIX>
          The prefix of the pgen file triples. There should be three files PFILE_PREFIX.pgen, PFILE_PREFIX.psam, and PFILE_PREFIX.pvar

Options:
  -f, --fstring <QUERY_FSTRING>
          An expression specifying what to output to stdout

  -i, --include <QUERY>
          An expression specifying which variants (default) or samples (if -s is passed) to keep

  -s, --samples
          When passed, the query is over the samples. Otherwise it is over the variants. Defaults false

  -h, --help
          Print help (see a summary with '-h')
```

#### Example queries

Print the chromosome and position (separated by space) of all variants which
have `G` as their alternate allele.

``` shell
$ pgen-rs query data/basic1/basic1 -i 'ALT == "G"' -f 'CHROM + " " + POS'
```

### `filter`
Filters the pgen, outputting to a VCF. Outputting to other formats is currently
not supported. Similar to [`bcftools
filter`](https://samtools.github.io/bcftools/bcftools.html#filter), but unlike
`pgen-rs query` the flags are different here; there are separate include
expression for the variants and samples.

All expressions have as variables the variant metadata. For example, if querying
the variants, `CHROM `and `ID `are variables which contain their respective values.

```
Usage: pgen-rs filter [OPTIONS] <PFILE_PREFIX>

Arguments:
  <PFILE_PREFIX>
          The prefix of the pgen file triples. There should be three files PFILE_PREFIX.pgen, PFILE_PREFIX.psam, and PFILE_PREFIX.pvar

Options:
      --include-var <VAR_QUERY>
          An expression specifying which variants to keep. If not passed, keeps all variants

      --include-sam <SAM_QUERY>
          An expression specifying which samples to keep. If not passed, keeps all samples

  -o, --out <OUT_FILE>
          The output file name (defaults to PFILE_PREFIX.pgen-rs.vcf)

  -h, --help
          Print help (see a summary with '-h')
```

#### Example queries

Filter the file to retain only the sample with ID `NA20900` and variants which
have `G` as their alternate allele, producing a VCF named `basic1.pgen-rs.vcf`.

``` shell
$ pgen-rs filter data/basic1/basic1 --include-sam 'IID == "NA20900"' --include-var 'ALT == "G"'
```

## `bcftools` comparison

We downloaded chr22 from the [1000Genomes
dataset](https://www.internationalgenome.org/) and ran some simple (and
unscientific) comparisons. Note that we did not make an index for `bcftools`,
which presumably improves its speed significantly.

We anticipate that `pgen-rs` is able to scale better than `bcftools` because of
two reasons.

1. When filtering by metadata, `pgen-rs` gets to quickly read the .pvar and
   .psam files which only contain the metadata and no hard calls.
2. When writing out rows and columns it gets to seek to them directly.

### Keep only two variants

~3s in `pgen-rs`:
``` shell
$ time pgen-rs filter data/chr22/chr22 --include-var 'POS=="16647494" || POS=="51241285"' -o data/chr22/chr22-filtered-pgen-rs.vcf
pgen-rs filter data/chr22/chr22 --include-var  -o   2.72s user 0.03s system 99% cpu 2.773 total
```

~40s in `bcftools`:
``` shell
$ time bcftools filter data/chr22/chr22.vcf.gz -i 'POS=16647494 || POS=51241285' -o data/chr22/chr22-filtered.vcf
bcftools filter data/chr22/chr22.vcf.gz -i 'POS=16647494 || POS=51241285' -o   39.76s user 0.07s system 99% cpu 39.857 total
```

### Keep all but two variants

~30s in `pgen-rs`:

``` shell
$ time pgen-rs filter data/chr22/chr22 --include-var 'POS!="16647494" || POS!="51241285"' -o data/chr22/chr22-filtered-pgen-rs.vcf
pgen-rs filter data/chr22/chr22 --include-var  -o   9.93s user 18.91s system 93% cpu 30.747 total
```

~90s in `bcftools`:
``` shell
$ time bcftools filter data/chr22/chr22.vcf.gz -i 'POS!=16647494 || POS!=51241285' -o data/chr22/chr22-filtered.vcf
bcftools filter data/chr22/chr22.vcf.gz -i 'POS!=16647494 || POS!=51241285' -  74.69s user 16.25s system 99% cpu 1:31.77 total
```


## The pgen format

The pgen format is described by its [official
specification](https://github.com/chrchang/plink-ng/tree/master/pgen_spec).

We provide an interface capable of parsing this format in
[src/pfile.rs](src/pfile.rs).

We currently only support Storage Mode `0x02`, which corresponds to a byte
matrix with hard calls for the unphased genotypes of the variants as rows and
samples as columns.

## Limitations

The pgen format was not designed to be optimized for read-only queries (see [its
limitations](https://www.cog-genomics.org/plink/1.9/#limits)).

However, in practice we notice that the VCF format often is queried anyway (e.g.
using bcftools). The pgen format is better for these types of queries due to the
way it splits up metadata and keeps its primary data in a format that makes it
easy to seek to arbitrary rows and columns.

`query` is currently separated into two separate queries on the variants or
samples. It does not support the ability to include genotypes in a variant query
which is perhaps the most useful thing to do. Supporting this would probably
require switching the underlying expression language with a custom one. If you
want to achieve this, the best thing to do would be to write a `filter` and then
use a tool like `bcftools query` on the output vcf.

## Expressions

We use the [`evalexpr`](https://github.com/ISibboI/evalexpr) expression language
for the include expressions as well as the format string in `query`.

Right now we do not parse the `INFO` or `FMT` column into its respective fields
(bcftools will for example let you use the variable `INFO/AF` to refer to the
`AF` field in the `INFO` column).

In general, this expression language has fewer domain-specific features than
bcftools's, but support for a few more functions by default. Future work
would be to support more of its domain-specific functions.

Also, all variables are strings (for now). We don't have any logic to parse a
numeric field into a numeric variable.
