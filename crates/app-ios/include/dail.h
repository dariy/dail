/*
 * dail's iOS host, libdail.a (crates/app-ios, ADR 0010). Written by hand; tests/it/header.rs
 * matches it against src/ffi.rs.
 *
 * Strings are NUL-terminated UTF-8. Strings the caller passes stay its own and need to live for
 * the call only. Strings and bytes returned are Rust's until handed back to dail_string_free and
 * dail_bytes_free. Every call answers JSON, {"error": "..."} on failure; none crashes on bad input.
 */
#ifndef DAIL_H
#define DAIL_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* wake(context) when a new frame is ready; alert(context, json) with each notification, json valid
 * for the call. Both are called from runtime threads. context must stay valid until dail_close. */
typedef struct DailListener {
    void *context;
    void (*wake)(void *context);
    void (*alert)(void *context, const char *json);
} DailListener;

/* Bytes owned by Rust; ptr is NULL (len 0) for none. */
typedef struct DailBytes {
    uint8_t *ptr;
    size_t len;
} DailBytes;

/* config: app_host::Config as JSON. Answers {"handle": n}. */
char *dail_open(const char *config, DailListener listener);
/* event: a UiEvent as JSON. Answers {"frame": ..., "externals": [...]}. */
char *dail_event(int32_t handle, const char *event);
/* Answers {"frame": ..., "externals": [...]}. */
char *dail_frame(int32_t handle);
/* request: a BlobRequest as JSON. Blocks; call it off the main thread. */
DailBytes dail_blob(int32_t handle, const char *request);
/* task: a background task, an import or a debug task as JSON. Blocks; call it off the main thread. */
char *dail_background(int32_t handle, const char *task);
/* Closes the host after its queued requests. */
void dail_close(int32_t handle);
void dail_string_free(char *text);
void dail_bytes_free(DailBytes bytes);

#ifdef __cplusplus
}
#endif

#endif /* DAIL_H */
