//! The ABI and version contract with the compiled kernel library.
//!
//! Three statements of that contract exist, and none of them is derived from
//! another: the constants written by hand in Rust, the macros written by hand
//! in the C header and mirrored here by the build script, and the values the
//! compiled library reports at run time. Each assertion below is between two
//! of the three, so a bump or a rename that reached one of them and not the
//! others fails here rather than at the first wrong number in a simulation.

use hydrax_kernels_sys::{
    ABI_VERSION, AbiVersion, CONSERVED_COUNT, KernelError, KernelStatus, Layout, backend_name,
    ensure_compatible, header, layout, library_accepts, library_status_message, library_version,
};

#[test]
fn the_crate_the_header_and_the_library_agree_on_the_abi_version() {
    assert_eq!(
        header::abi_version(),
        ABI_VERSION,
        "the C header and this crate declare different ABI versions"
    );
    assert_eq!(
        library_version(),
        ABI_VERSION,
        "the compiled kernel library reports an ABI version this crate does not expect"
    );
}

#[test]
fn the_crate_and_the_header_agree_on_the_conserved_variable_count() {
    assert_eq!(
        header::CONSERVED_COUNT,
        CONSERVED_COUNT,
        "the C header and this crate disagree about the conserved-variable count"
    );
    assert_eq!(
        usize::try_from(layout().conserved_count).expect("a count that fits a u32 fits a usize"),
        CONSERVED_COUNT,
        "the compiled kernel library was built for a different conserved-variable count"
    );
}

#[test]
fn the_library_and_the_crate_agree_on_every_shared_memory_layout() {
    let library = layout();
    let expected = Layout::expected();
    // Compared field by field first, so a failure names the descriptor that
    // drifted rather than printing two records to be diffed by eye.
    for (name, found, want) in [
        (
            "hydrax_kernels_layout size",
            library.size_of_layout,
            expected.size_of_layout,
        ),
        (
            "hydrax_kernels_layout alignment",
            library.align_of_layout,
            expected.align_of_layout,
        ),
        (
            "hydrax_kernels_riemann_input size",
            library.size_of_riemann_input,
            expected.size_of_riemann_input,
        ),
        (
            "hydrax_kernels_riemann_input alignment",
            library.align_of_riemann_input,
            expected.align_of_riemann_input,
        ),
        (
            "hydrax_kernels_shell_input size",
            library.size_of_shell_input,
            expected.size_of_shell_input,
        ),
        (
            "hydrax_kernels_shell_input alignment",
            library.align_of_shell_input,
            expected.align_of_shell_input,
        ),
        ("double size", library.size_of_real, expected.size_of_real),
        ("size_t size", library.size_of_index, expected.size_of_index),
    ] {
        assert_eq!(found, want, "{name} differs between the library and Rust");
    }
    assert_eq!(library, expected);
}

#[test]
fn the_index_type_of_the_abi_is_the_one_rust_passes() {
    // `size_t` and `usize` are separate promises. The pointers and lengths of
    // every kernel call are formed from Rust slices, so a target where the two
    // widths differ would corrupt every length crossing the boundary.
    assert_eq!(
        layout().size_of_index,
        u64::try_from(size_of::<usize>()).expect("a type size fits a u64")
    );
    assert_eq!(
        layout().size_of_real,
        u64::try_from(size_of::<f64>()).expect("a type size fits a u64")
    );
}

#[test]
fn the_library_applies_the_compatibility_rule_this_crate_documents() {
    let library = library_version();
    for major in [0_u8, 1, 2, 255] {
        for minor in [0_u8, 1, 2, 255] {
            for patch in [0_u8, 1, 255] {
                let caller = AbiVersion::new(major, minor, patch);
                let accepted = library_accepts(caller).is_ok();
                assert_eq!(
                    accepted,
                    library.serves(caller),
                    "the library and `AbiVersion::serves` disagree about caller {caller} \
                     against library {library}"
                );
            }
        }
    }
}

#[test]
fn an_incompatible_caller_is_refused_with_the_abi_mismatch_status() {
    let library = library_version();
    let wrong_major = AbiVersion::new(library.major.wrapping_add(1), library.minor, library.patch);
    assert_eq!(
        library_accepts(wrong_major),
        Err(KernelError::Rejected(KernelStatus::AbiMismatch))
    );

    let unreleased_minor = AbiVersion::new(library.major, 255, library.patch);
    assert!(
        unreleased_minor.minor > library.minor,
        "this test needs a minor component the library cannot have"
    );
    assert_eq!(
        library_accepts(unreleased_minor),
        Err(KernelError::Rejected(KernelStatus::AbiMismatch))
    );
}

#[test]
fn the_linked_library_is_usable_by_this_crate() {
    assert_eq!(ensure_compatible(), Ok(()));
}

#[test]
fn the_library_defines_exactly_the_status_codes_this_crate_knows() {
    let mut messages = Vec::new();
    for status in KernelStatus::ALL {
        let message = library_status_message(status.code()).unwrap_or_else(|| {
            panic!(
                "the kernel library does not define status {status:?} ({})",
                status.code()
            )
        });
        assert!(
            !message.is_empty(),
            "status {status:?} has an empty message"
        );
        messages.push(message);
    }
    // Distinct messages are what makes the codes distinguishable in a log; a
    // duplicate usually means a code was copied and its message was not
    // updated.
    for (position, message) in messages.iter().enumerate() {
        assert!(
            !messages[position + 1..].contains(message),
            "two statuses share the message {message:?}"
        );
    }
    for undefined in [-1, 7, 8, i32::MIN, i32::MAX] {
        assert_eq!(
            library_status_message(undefined),
            None,
            "the kernel library defines status {undefined}, which this crate does not"
        );
    }
}

#[test]
fn the_library_and_this_crate_describe_each_status_the_same_way() {
    for status in KernelStatus::ALL {
        let described = status.to_string();
        assert_eq!(
            library_status_message(status.code()),
            Some(described.as_str()),
            "the descriptions of {status:?} have drifted apart"
        );
    }
}

#[test]
fn the_backend_names_itself() {
    let name = backend_name();
    assert!(!name.is_empty(), "the backend reported an empty name");
    assert!(
        name.chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character)),
        "the backend name {name:?} is not a plain identifier fit for a provenance record"
    );
}
