#pragma once
#include <stdint.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef struct { void* ptr; } QnetConn;
typedef struct { void* ptr; } QnetStream;

int qnet_dial_inproc(QnetConn* out_client, QnetConn* out_server);

QnetStream qnet_conn_open_stream(QnetConn* conn);
QnetStream qnet_conn_accept_stream(QnetConn* conn, uint64_t timeout_ms);

int qnet_stream_write(QnetStream* st, const uint8_t* data, size_t len);
intptr_t qnet_stream_read(QnetStream* st, uint8_t* out_buf, size_t cap);

void qnet_conn_free(QnetConn* conn);
void qnet_stream_free(QnetStream* st);

#ifdef __cplusplus
}
#endif
