param(
    [int]$Port = 9443
)

$ErrorActionPreference = 'Stop'

Write-Host "Starting HTTP /health server on 127.0.0.1:$Port (Ctrl+C to stop)"

$listener = New-Object System.Net.Sockets.TcpListener ([System.Net.IPAddress]::Loopback, $Port)
$listener.Start()
try {
    while ($true) {
        $client = $listener.AcceptTcpClient()
        try {
            $client.ReceiveTimeout = 5000
            $client.SendTimeout = 5000
            $stream = $client.GetStream()
            $writer = New-Object System.IO.StreamWriter($stream)
            $writer.NewLine = "`r`n"
            # Respond immediately with 200 OK and body 'ok' to avoid browser timeouts on read
            $body = "ok"
            $resp = "HTTP/1.1 200 OK`r`nContent-Type: text/plain`r`nContent-Length: {0}`r`nConnection: close`r`n`r`n{1}" -f $body.Length, $body
            $writer.Write($resp)
            $writer.Flush()
            Write-Host "[+] Served /health -> 200"
            $client.Close()
        } catch {
            try { $client.Close() } catch {}
        }
    }
} finally {
    $listener.Stop()
}
