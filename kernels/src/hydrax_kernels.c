/*
 * ABI introspection for the Hydrax kernel library.
 *
 * The version and layout the library reports are compiled from the same header
 * every caller includes, so a library built from a stale header reports the
 * stale numbers rather than agreeing by construction. That is the whole point:
 * these functions exist to be disagreed with.
 */

#include "hydrax_kernels.h"

#include <stddef.h>
#include <stdint.h>

/*
 * `_Alignof` is a C11 keyword, but a compiler in a pre-C11 mode -- or an older
 * MSVC -- offers the same query under its own spelling. Alignment is part of
 * the layout contract, so it is queried rather than assumed.
 */
#if defined(_MSC_VER) && !defined(__clang__)
#define HYDRAX_KERNELS_ALIGNOF(type) __alignof(type)
#elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
#define HYDRAX_KERNELS_ALIGNOF(type) _Alignof(type)
#else
#define HYDRAX_KERNELS_ALIGNOF(type) __alignof__(type)
#endif

/** Name reported for the portable CPU implementation of the kernels. */
static const char HYDRAX_KERNELS_BACKEND[] = "reference-c11";

uint32_t hydrax_kernels_abi_version(void) { return HYDRAX_KERNELS_ABI_VERSION; }

hydrax_kernels_status hydrax_kernels_abi_check(uint32_t caller_version) {
    const uint32_t library = HYDRAX_KERNELS_ABI_VERSION;
    if (HYDRAX_KERNELS_ABI_MAJOR_OF(caller_version) !=
        HYDRAX_KERNELS_ABI_MAJOR_OF(library)) {
        return HYDRAX_KERNELS_ERR_ABI_MISMATCH;
    }
    if (HYDRAX_KERNELS_ABI_MINOR_OF(library) <
        HYDRAX_KERNELS_ABI_MINOR_OF(caller_version)) {
        return HYDRAX_KERNELS_ERR_ABI_MISMATCH;
    }
    return HYDRAX_KERNELS_OK;
}

hydrax_kernels_status hydrax_kernels_layout_query(hydrax_kernels_layout *layout) {
    if (layout == NULL) {
        return HYDRAX_KERNELS_ERR_NULL_POINTER;
    }
    layout->abi_version = HYDRAX_KERNELS_ABI_VERSION;
    layout->conserved_count = (uint32_t)HYDRAX_KERNELS_CONSERVED_COUNT;
    layout->size_of_layout = (uint64_t)sizeof(hydrax_kernels_layout);
    layout->align_of_layout =
        (uint64_t)HYDRAX_KERNELS_ALIGNOF(hydrax_kernels_layout);
    layout->size_of_riemann_input =
        (uint64_t)sizeof(hydrax_kernels_riemann_input);
    layout->align_of_riemann_input =
        (uint64_t)HYDRAX_KERNELS_ALIGNOF(hydrax_kernels_riemann_input);
    layout->size_of_shell_input = (uint64_t)sizeof(hydrax_kernels_shell_input);
    layout->align_of_shell_input =
        (uint64_t)HYDRAX_KERNELS_ALIGNOF(hydrax_kernels_shell_input);
    layout->size_of_real = (uint64_t)sizeof(double);
    layout->size_of_index = (uint64_t)sizeof(size_t);
    return HYDRAX_KERNELS_OK;
}

const char *hydrax_kernels_backend_name(void) { return HYDRAX_KERNELS_BACKEND; }

const char *hydrax_kernels_status_message(hydrax_kernels_status status) {
    switch (status) {
    case HYDRAX_KERNELS_OK:
        return "the call succeeded";
    case HYDRAX_KERNELS_ERR_NULL_POINTER:
        return "a required buffer pointer was null";
    case HYDRAX_KERNELS_ERR_EMPTY_DOMAIN:
        return "the domain held no zones";
    case HYDRAX_KERNELS_ERR_ABI_MISMATCH:
        return "the caller and the kernel library disagree about the ABI";
    case HYDRAX_KERNELS_ERR_INVALID_GEOMETRY:
        return "a cell volume or interface area was not a usable geometric "
               "measure";
    case HYDRAX_KERNELS_ERR_DEGENERATE_WAVE_SPEEDS:
        return "a signal-speed pair was not a finite, correctly ordered "
               "interval";
    case HYDRAX_KERNELS_ERR_SIZE_OVERFLOW:
        return "the requested domain size cannot be addressed on this target";
    default:
        return NULL;
    }
}
