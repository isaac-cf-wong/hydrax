/*
 * Portable CPU implementation of the Hydrax hydrodynamics kernels.
 *
 * This backend is written for clarity and portability rather than speed: it is
 * the implementation a GPU backend has to reproduce, and the one the Rust
 * differential tests compare against. Its arithmetic is written in the same
 * order as the safe-Rust reference in `hydrax-kernels-sys`, so the two agree to
 * within a few units in the last place rather than merely to plotting
 * accuracy.
 */

#include "hydrax_kernels.h"

#include <math.h>
#include <stddef.h>
#include <stdint.h>

/** Conserved-variable stride of every state and flux buffer. */
#define HYDRAX_KERNELS_STRIDE ((size_t)HYDRAX_KERNELS_CONSERVED_COUNT)

/**
 * Report whether `count * HYDRAX_KERNELS_STRIDE` is representable.
 *
 * A caller that cannot have allocated the buffer it describes is refused
 * before any index is formed, rather than after the multiplication has already
 * wrapped.
 */
static int hydrax_kernels_stride_fits(size_t count) {
    return count <= SIZE_MAX / HYDRAX_KERNELS_STRIDE;
}

/**
 * Return the argument of smaller magnitude when both share a sign, else zero.
 *
 * A NaN argument compares false against both zero tests and so limits the
 * slope to zero, which is the conservative answer for a limiter.
 */
static double hydrax_kernels_minmod(double left, double right) {
    if (left > 0.0 && right > 0.0) {
        return (left < right) ? left : right;
    }
    if (left < 0.0 && right < 0.0) {
        return (left > right) ? left : right;
    }
    return 0.0;
}

hydrax_kernels_status hydrax_kernels_minmod_slopes(const double *values,
                                                   size_t cell_count,
                                                   double *slopes) {
    if (values == NULL || slopes == NULL) {
        return HYDRAX_KERNELS_ERR_NULL_POINTER;
    }
    if (cell_count == 0u) {
        return HYDRAX_KERNELS_ERR_EMPTY_DOMAIN;
    }

    slopes[0] = 0.0;
    if (cell_count == 1u) {
        return HYDRAX_KERNELS_OK;
    }
    slopes[cell_count - 1u] = 0.0;
    for (size_t cell = 1u; cell + 1u < cell_count; ++cell) {
        const double backward = values[cell] - values[cell - 1u];
        const double forward = values[cell + 1u] - values[cell];
        slopes[cell] = hydrax_kernels_minmod(backward, forward);
    }
    return HYDRAX_KERNELS_OK;
}

hydrax_kernels_status
hydrax_kernels_hll_flux(const hydrax_kernels_riemann_input *input,
                        double *flux) {
    if (input == NULL || flux == NULL) {
        return HYDRAX_KERNELS_ERR_NULL_POINTER;
    }
    if (input->left_state == NULL || input->right_state == NULL ||
        input->left_flux == NULL || input->right_flux == NULL ||
        input->left_speed == NULL || input->right_speed == NULL) {
        return HYDRAX_KERNELS_ERR_NULL_POINTER;
    }
    if (input->interface_count == 0u) {
        return HYDRAX_KERNELS_ERR_EMPTY_DOMAIN;
    }
    if (!hydrax_kernels_stride_fits(input->interface_count)) {
        return HYDRAX_KERNELS_ERR_SIZE_OVERFLOW;
    }

    /*
     * The speeds are checked in full before a single flux is written, so a
     * rejected call leaves the caller's output buffer as it found it.
     */
    for (size_t face = 0u; face < input->interface_count; ++face) {
        const double slowest = input->left_speed[face];
        const double fastest = input->right_speed[face];
        if (!isfinite(slowest) || !isfinite(fastest) || !(slowest <= fastest)) {
            return HYDRAX_KERNELS_ERR_DEGENERATE_WAVE_SPEEDS;
        }
    }

    for (size_t face = 0u; face < input->interface_count; ++face) {
        const double slowest = input->left_speed[face];
        const double fastest = input->right_speed[face];
        const size_t base = face * HYDRAX_KERNELS_STRIDE;
        for (size_t component = 0u; component < HYDRAX_KERNELS_STRIDE;
             ++component) {
            const size_t index = base + component;
            const double left_flux = input->left_flux[index];
            const double right_flux = input->right_flux[index];
            if (slowest >= 0.0) {
                flux[index] = left_flux;
            } else if (fastest <= 0.0) {
                flux[index] = right_flux;
            } else {
                const double jump =
                    input->right_state[index] - input->left_state[index];
                flux[index] = (fastest * left_flux - slowest * right_flux +
                               slowest * fastest * jump) /
                              (fastest - slowest);
            }
        }
    }
    return HYDRAX_KERNELS_OK;
}

hydrax_kernels_status
hydrax_kernels_spherical_flux_divergence(
    const hydrax_kernels_shell_input *input, double *rhs) {
    if (input == NULL || rhs == NULL) {
        return HYDRAX_KERNELS_ERR_NULL_POINTER;
    }
    if (input->interface_flux == NULL || input->interface_area == NULL ||
        input->cell_volume == NULL || input->source == NULL) {
        return HYDRAX_KERNELS_ERR_NULL_POINTER;
    }
    if (input->cell_count == 0u) {
        return HYDRAX_KERNELS_ERR_EMPTY_DOMAIN;
    }
    /*
     * One more interface than cells, and both buffers carry the conserved
     * stride, so the interface count is the quantity that has to fit.
     */
    if (input->cell_count == SIZE_MAX ||
        !hydrax_kernels_stride_fits(input->cell_count + 1u)) {
        return HYDRAX_KERNELS_ERR_SIZE_OVERFLOW;
    }

    for (size_t face = 0u; face < input->cell_count + 1u; ++face) {
        const double area = input->interface_area[face];
        if (!isfinite(area) || area < 0.0) {
            return HYDRAX_KERNELS_ERR_INVALID_GEOMETRY;
        }
    }
    for (size_t cell = 0u; cell < input->cell_count; ++cell) {
        const double volume = input->cell_volume[cell];
        if (!isfinite(volume) || !(volume > 0.0)) {
            return HYDRAX_KERNELS_ERR_INVALID_GEOMETRY;
        }
    }

    for (size_t cell = 0u; cell < input->cell_count; ++cell) {
        const double inner_area = input->interface_area[cell];
        const double outer_area = input->interface_area[cell + 1u];
        const double volume = input->cell_volume[cell];
        const size_t inner = cell * HYDRAX_KERNELS_STRIDE;
        const size_t outer = (cell + 1u) * HYDRAX_KERNELS_STRIDE;
        for (size_t component = 0u; component < HYDRAX_KERNELS_STRIDE;
             ++component) {
            const double inner_flux = input->interface_flux[inner + component];
            const double outer_flux = input->interface_flux[outer + component];
            rhs[inner + component] =
                input->source[inner + component] -
                (outer_area * outer_flux - inner_area * inner_flux) / volume;
        }
    }
    return HYDRAX_KERNELS_OK;
}
