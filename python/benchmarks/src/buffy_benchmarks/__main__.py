"""Runs the benchmarks with pyperf, which takes its own options too. See
README.md for full and quick runs."""

import argparse
import re
import sysconfig

import pyperf

from buffy_benchmarks.suite import groups, time_variant


def main() -> None:
    runner = pyperf.Runner(
        program_args=("-m", "buffy_benchmarks"),
        add_cmdline_args=_add_worker_options,
        metadata={"python_build": _python_build()},
    )
    runner.argparser.add_argument(
        "--filter",
        default="",
        metavar="REGEX",
        help="run only the groups whose name matches",
    )
    runner.argparser.add_argument(
        "--reverse",
        action="store_true",
        help="run each group's codecs in reverse order",
    )
    args = runner.parse_args()
    pattern = re.compile(args.filter)

    # A worker registers the same benchmarks in the same order, and runs the
    # one whose index it was given
    for group in groups():
        if not pattern.search(group.name):
            continue

        variants = reversed(group.variants) if args.reverse else group.variants
        for variant in variants:
            runner.bench_time_func(
                f"{variant.codec}: {group.name}",
                time_variant,
                variant.prepare,
                metadata={"group": group.name, "codec": variant.codec},
            )


def _add_worker_options(command: list[str], args: argparse.Namespace) -> None:
    command.extend(("--filter", args.filter))
    if args.reverse:
        command.append("--reverse")


def _python_build() -> str:
    """Whether the interpreter was built with the GIL or free-threaded"""
    return "free-threaded" if sysconfig.get_config_var("Py_GIL_DISABLED") else "GIL"


if __name__ == "__main__":
    main()
