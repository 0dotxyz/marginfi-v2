#!/usr/bin/env python3
"""
Build a minimal Kamino IDL for Anchor `declare_program!` macro generation.

This script keeps only the CPI instruction surface used by marginfi and prunes
unneeded account/type definitions to reduce compile-time bloat.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import subprocess
import tempfile
from pathlib import Path
from typing import Any


LENDING_KEEP_CANONICAL = {
    "refresh_reserve",
    "refresh_reserves_batch",
    "init_user_metadata",
    "init_obligation",
    "init_obligation_farms_for_reserve",
    "refresh_obligation",
    "deposit_reserve_liquidity_and_obligation_collateral",
    "deposit_reserve_liquidity_and_obligation_collateral_v2",
    "withdraw_obligation_collateral_and_redeem_reserve_collateral",
    "withdraw_obligation_collateral_and_redeem_reserve_collateral_v2",
}

FARMS_KEEP_CANONICAL = {
    "harvest_reward",
}

DEFAULT_PROGRAM_ID_BY_PROFILE = {
    "lending": "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD",
    "farms": "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr",
}

DEFAULT_PROGRAM_NAME_BY_PROFILE = {
    "lending": "kamino_lending",
    "farms": "farms",
}


def to_snake(name: str) -> str:
    name = name.replace("-", "_")
    if "_" in name:
        return name.lower()
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def normalize_instruction_name(name: str) -> str:
    return to_snake(name)


def extract_defined_name(type_spec: Any) -> str | None:
    if not isinstance(type_spec, dict):
        return None
    defined = type_spec.get("defined")
    if isinstance(defined, str):
        return defined
    if isinstance(defined, dict):
        val = defined.get("name")
        if isinstance(val, str):
            return val
    return None


def collect_defined_types_from_type_spec(type_spec: Any, out: set[str]) -> None:
    defined = extract_defined_name(type_spec)
    if defined:
        out.add(defined)

    if isinstance(type_spec, dict):
        for key in ("option", "vec"):
            if key in type_spec:
                collect_defined_types_from_type_spec(type_spec[key], out)

        if "array" in type_spec:
            arr = type_spec["array"]
            # Anchor IDL array representation: [inner_type, length]
            if isinstance(arr, list) and arr:
                collect_defined_types_from_type_spec(arr[0], out)

        if "tuple" in type_spec and isinstance(type_spec["tuple"], list):
            for item in type_spec["tuple"]:
                collect_defined_types_from_type_spec(item, out)


def canonicalize_type_spec(type_spec: Any) -> Any:
    if isinstance(type_spec, str):
        if type_spec == "publicKey":
            return "pubkey"
        return type_spec

    if not isinstance(type_spec, dict):
        return type_spec

    defined = type_spec.get("defined")
    if isinstance(defined, str):
        type_spec["defined"] = {"name": defined}

    for key in ("option", "vec"):
        if key in type_spec:
            type_spec[key] = canonicalize_type_spec(type_spec[key])

    if "array" in type_spec:
        arr = type_spec["array"]
        if isinstance(arr, list) and arr:
            arr[0] = canonicalize_type_spec(arr[0])

    if "tuple" in type_spec and isinstance(type_spec["tuple"], list):
        for i, item in enumerate(type_spec["tuple"]):
            type_spec["tuple"][i] = canonicalize_type_spec(item)

    return type_spec


def canonicalize_type_def(type_def: dict[str, Any]) -> None:
    body = type_def.get("type")
    if not isinstance(body, dict):
        return

    kind = body.get("kind")
    if kind == "struct":
        fields = body.get("fields")
        if isinstance(fields, list):
            for field in fields:
                if isinstance(field, dict) and "type" in field:
                    field["type"] = canonicalize_type_spec(field["type"])
                else:
                    canonicalize_type_spec(field)
    elif kind == "enum":
        variants = body.get("variants", [])
        if isinstance(variants, list):
            for variant in variants:
                if not isinstance(variant, dict):
                    continue
                fields = variant.get("fields")
                if isinstance(fields, list):
                    for field in fields:
                        if isinstance(field, dict) and "type" in field:
                            field["type"] = canonicalize_type_spec(field["type"])
                        else:
                            canonicalize_type_spec(field)


def collect_defined_types_from_fields(fields: Any, out: set[str]) -> None:
    if not isinstance(fields, list):
        return

    for field in fields:
        if isinstance(field, dict) and "type" in field:
            collect_defined_types_from_type_spec(field["type"], out)
        else:
            # tuple-like enum fields may be plain type specs
            collect_defined_types_from_type_spec(field, out)


def collect_type_dependencies(type_def: dict[str, Any], out: set[str]) -> None:
    body = type_def.get("type")
    if not isinstance(body, dict):
        return

    kind = body.get("kind")
    if kind == "struct":
        collect_defined_types_from_fields(body.get("fields"), out)
    elif kind == "enum":
        variants = body.get("variants", [])
        if isinstance(variants, list):
            for variant in variants:
                if isinstance(variant, dict):
                    collect_defined_types_from_fields(variant.get("fields"), out)


def rename_v2_farms_groups(instruction: dict[str, Any]) -> None:
    inst_name = normalize_instruction_name(str(instruction.get("name", "")))
    if inst_name not in {
        "deposit_reserve_liquidity_and_obligation_collateral_v2",
        "withdraw_obligation_collateral_and_redeem_reserve_collateral_v2",
    }:
        return

    expected = (
        "deposit_farms_accounts"
        if inst_name == "deposit_reserve_liquidity_and_obligation_collateral_v2"
        else "withdraw_farms_accounts"
    )

    for account in instruction.get("accounts", []):
        if not isinstance(account, dict):
            continue
        name = account.get("name")
        if not isinstance(name, str):
            continue
        if name in ("farms_accounts", "farmsAccounts"):
            account["name"] = expected


def normalize_account_names(accounts: Any) -> None:
    if not isinstance(accounts, list):
        return
    for account in accounts:
        if not isinstance(account, dict):
            continue
        name = account.get("name")
        if isinstance(name, str):
            account["name"] = to_snake(name)

        if "isMut" in account and "writable" not in account:
            account["writable"] = bool(account["isMut"])
        if "isSigner" in account and "signer" not in account:
            account["signer"] = bool(account["isSigner"])
        if "isOptional" in account and "optional" not in account:
            account["optional"] = bool(account["isOptional"])

        account.pop("isMut", None)
        account.pop("isSigner", None)
        account.pop("isOptional", None)

        if "accounts" in account:
            normalize_account_names(account["accounts"])


def compute_instruction_discriminator(ix_name_snake: str) -> list[int]:
    preimage = f"global:{ix_name_snake}".encode("utf-8")
    digest = hashlib.sha256(preimage).digest()
    return list(digest[:8])


def canonicalize_instruction_shape(instruction: dict[str, Any]) -> None:
    name = instruction.get("name")
    if isinstance(name, str):
        instruction["name"] = normalize_instruction_name(name)

    normalize_account_names(instruction.get("accounts"))

    for arg in instruction.get("args", []):
        if isinstance(arg, dict):
            arg_name = arg.get("name")
            if isinstance(arg_name, str):
                arg["name"] = to_snake(arg_name)
            if "type" in arg:
                arg["type"] = canonicalize_type_spec(arg["type"])

    if "discriminator" not in instruction:
        ix_name = instruction.get("name")
        if isinstance(ix_name, str):
            instruction["discriminator"] = compute_instruction_discriminator(ix_name)


def select_instructions(
    idl: dict[str, Any], keep_canonical: set[str]
) -> list[dict[str, Any]]:
    selected: list[dict[str, Any]] = []
    for ix in idl.get("instructions", []):
        if not isinstance(ix, dict):
            continue
        name = ix.get("name")
        if not isinstance(name, str):
            continue
        if normalize_instruction_name(name) in keep_canonical:
            ix_copy = copy.deepcopy(ix)
            rename_v2_farms_groups(ix_copy)
            canonicalize_instruction_shape(ix_copy)
            selected.append(ix_copy)
    return selected


def prune_types(
    idl: dict[str, Any], selected_instructions: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    all_types = idl.get("types", [])
    if not isinstance(all_types, list) or not all_types:
        return []

    by_name: dict[str, dict[str, Any]] = {}
    ordered_names: list[str] = []
    for type_def in all_types:
        if not isinstance(type_def, dict):
            continue
        name = type_def.get("name")
        if isinstance(name, str):
            by_name[name] = type_def
            ordered_names.append(name)

    needed: set[str] = set()
    for ix in selected_instructions:
        for arg in ix.get("args", []):
            if isinstance(arg, dict):
                collect_defined_types_from_type_spec(arg.get("type"), needed)

    queue = list(needed)
    while queue:
        type_name = queue.pop()
        type_def = by_name.get(type_name)
        if not type_def:
            continue
        deps: set[str] = set()
        collect_type_dependencies(type_def, deps)
        for dep in deps:
            if dep not in needed:
                needed.add(dep)
                queue.append(dep)

    selected = [copy.deepcopy(by_name[name]) for name in ordered_names if name in needed]
    for type_def in selected:
        canonicalize_type_def(type_def)
    return selected


def distill_idl(
    idl: dict[str, Any], keep_canonical: set[str], drop_top_level_accounts: bool
) -> dict[str, Any]:
    out = copy.deepcopy(idl)

    selected_instructions = select_instructions(out, keep_canonical)
    out["instructions"] = selected_instructions
    out["types"] = prune_types(out, selected_instructions)

    if drop_top_level_accounts:
        out["accounts"] = []

    # These are not needed for CPI/macro generation and add extra generated code.
    out["events"] = []

    return out


def ensure_anchor_metadata(
    idl: dict[str, Any], profile: str, program_id_override: str | None
) -> None:
    """
    Ensure the distilled IDL has the fields expected by Anchor's `convert_idl`.
    """
    program_id = (
        program_id_override
        or idl.get("address")
        or DEFAULT_PROGRAM_ID_BY_PROFILE.get(profile)
    )
    if isinstance(program_id, str):
        idl["address"] = program_id

    metadata = idl.get("metadata")
    if not isinstance(metadata, dict):
        metadata = {}

    if not isinstance(metadata.get("name"), str):
        metadata["name"] = idl.get("name") or DEFAULT_PROGRAM_NAME_BY_PROFILE.get(profile)
    if not isinstance(metadata.get("version"), str):
        metadata["version"] = idl.get("version") or "0.0.0"
    if not isinstance(metadata.get("spec"), str):
        metadata["spec"] = "0.1.0"
    if isinstance(program_id, str):
        metadata["address"] = program_id

    idl["metadata"] = metadata


def needs_anchor_convert(idl: dict[str, Any]) -> bool:
    instructions = idl.get("instructions")
    if not isinstance(instructions, list):
        return False
    for ix in instructions:
        if isinstance(ix, dict) and "discriminator" not in ix:
            return True
    return False


def run_anchor_idl_convert(input_path: Path, program_id: str | None) -> dict[str, Any]:
    with tempfile.NamedTemporaryFile("w+", suffix=".json") as tmp:
        cmd = ["anchor", "idl", "convert", str(input_path), "-o", tmp.name]
        if isinstance(program_id, str) and program_id:
            cmd.extend(["--program-id", program_id])

        subprocess.run(cmd, check=True)
        tmp.seek(0)
        return json.load(tmp)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, help="Input IDL JSON path")
    parser.add_argument("--output", required=True, help="Output IDL JSON path")
    parser.add_argument(
        "--profile",
        required=True,
        choices=["lending", "farms"],
        help="Distillation profile",
    )
    parser.add_argument(
        "--keep-top-level-accounts",
        action="store_true",
        help="Keep top-level .accounts instead of clearing them",
    )
    parser.add_argument(
        "--program-id",
        help="Optional program id override for output address/metadata.address",
    )
    parser.add_argument(
        "--skip-anchor-convert",
        action="store_true",
        help="Skip pre-converting raw IDLs via `anchor idl convert`",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    input_path = Path(args.input)
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    keep_canonical = (
        LENDING_KEEP_CANONICAL if args.profile == "lending" else FARMS_KEEP_CANONICAL
    )
    program_id = args.program_id or DEFAULT_PROGRAM_ID_BY_PROFILE.get(args.profile)

    with input_path.open("r", encoding="utf-8") as f:
        idl = json.load(f)

    if not args.skip_anchor_convert and needs_anchor_convert(idl):
        idl = run_anchor_idl_convert(input_path, program_id)

    distilled = distill_idl(
        idl=idl,
        keep_canonical=keep_canonical,
        drop_top_level_accounts=not args.keep_top_level_accounts,
    )
    ensure_anchor_metadata(distilled, args.profile, program_id)

    with output_path.open("w", encoding="utf-8") as f:
        json.dump(distilled, f, indent=2, sort_keys=False)
        f.write("\n")

    print(f"Wrote {output_path}")
    print("Instructions kept:")
    for ix in distilled.get("instructions", []):
        print(f"  - {ix.get('name')}")
    print(f"Top-level accounts: {len(distilled.get('accounts', []))}")
    print(f"Types: {len(distilled.get('types', []))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
