#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include "../include/qnet.h"

int main(void) {
    QnetConn client = {0}, server = {0};
    if (qnet_dial_inproc(&client, &server) != 0) {
        fprintf(stderr, "qnet_dial_inproc failed\n");
        return 1;
    }
    // server side: accept a stream and echo a single message
    QnetStream ss = qnet_conn_accept_stream(&server, 1000);
    if (ss.ptr == NULL) {
        fprintf(stderr, "server accept failed\n");
        return 2;
    }

    // client side: open a stream and send a message
    QnetStream cs = qnet_conn_open_stream(&client);
    const char* msg = "hello-c-lib";
    if (qnet_stream_write(&cs, (const uint8_t*)msg, (size_t)strlen(msg)) != 0) {
        fprintf(stderr, "write failed\n");
        return 3;
    }

    // server reads and echoes back
    uint8_t buf[256];
    intptr_t n = qnet_stream_read(&ss, buf, sizeof(buf));
    if (n <= 0) {
        fprintf(stderr, "server read failed (%ld)\n", (long)n);
        return 4;
    }
    if (qnet_stream_write(&ss, buf, (size_t)n) != 0) {
        fprintf(stderr, "server echo write failed\n");
        return 5;
    }

    // client reads echoed message
    uint8_t out[256];
    intptr_t rn = qnet_stream_read(&cs, out, sizeof(out));
    if (rn <= 0) {
        fprintf(stderr, "client read failed (%ld)\n", (long)rn);
        return 6;
    }
    out[rn] = '\0';
    printf("echoed: %s\n", (char*)out);

    qnet_stream_free(&cs);
    qnet_stream_free(&ss);
    qnet_conn_free(&client);
    qnet_conn_free(&server);
    return 0;
}
