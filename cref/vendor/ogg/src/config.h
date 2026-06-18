/* Minimal libogg config.h for the libflac-rs `cref` oracle build.
 *
 * The build defines HAVE_CONFIG_H globally (libFLAC needs it), so libogg's
 * framing.c / bitwise.c do `#include "config.h"`. This local stub satisfies that
 * quote-include (resolved from the including file's own directory first) so libogg
 * does NOT pick up libFLAC's config.h. libogg derives its fixed-width types from
 * <ogg/os_types.h> via platform macros, so nothing is needed here on glibc.
 *
 * Dev-only: this lives under cref/, which is excluded from the published crate.
 */
