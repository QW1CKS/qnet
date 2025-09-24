param([string]$Port = "8088")

$client = $null
try {
    $client = New-Object System.Net.Sockets.TcpClient
    $client.Connect("127.0.0.1", [int]$Port)
    
    $stream = $client.GetStream()
    $request = "GET /status HTTP/1.1`r`nHost: 127.0.0.1:$Port`r`nConnection: close`r`n`r`n"
    $data = [System.Text.Encoding]::ASCII.GetBytes($request)
    $stream.Write($data, 0, $data.Length)
    
    $responseData = New-Object System.Collections.Generic.List[byte]
    $buffer = New-Object byte[] 1024
    
    do {
        $bytesRead = $stream.Read($buffer, 0, $buffer.Length)
        if ($bytesRead -gt 0) {
            for ($i = 0; $i -lt $bytesRead; $i++) {
                $responseData.Add($buffer[$i])
            }
        }
    } while ($bytesRead -gt 0)
    
    $response = [System.Text.Encoding]::ASCII.GetString($responseData.ToArray())
    Write-Output "HTTP Response:"
    Write-Output $response
    
    # Extract JSON from response
    $jsonStart = $response.IndexOf('{')
    if ($jsonStart -ge 0) {
        $json = $response.Substring($jsonStart)
        Write-Output "`nParsed JSON:"
        $jsonObj = $json | ConvertFrom-Json
        $jsonObj | ConvertTo-Json -Depth 10
    }
    
} catch {
    Write-Error "Error connecting to status server: $($_.Exception.Message)"
} finally {
    if ($client) {
        $client.Close()
    }
}