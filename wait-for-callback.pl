use strict;
use warnings;
use IO::Socket::INET;

# Auto-flush socket
$| = 1;

# Creating a listening TCP socket on port 9800
my $socket = new IO::Socket::INET(
    LocalHost => '127.0.0.1',
    LocalPort => '9800',
    Proto => 'tcp',
    Listen => 5,
    Reuse => 1
) or die "Could not create socket: $!\n";

print "Server waiting for client connection on port 9800\n";

# Accepting a connection
my $client_socket = $socket->accept();

# Initial read for headers
my $headers = "";
while (my $line = <$client_socket>) {
    last if $line =~ /^\r\n$/; # End of headers
    $headers .= $line;
    if ($line =~ /^Content-Length: (\d+)/i) {
        my $content_length = $1;

        # Read the body based on Content-Length
        my $crlf;
        read($client_socket, $crlf, 2);
        my $body;
        read($client_socket, $body, $content_length);

        print "$body\n";
        $client_socket->close();
        exit 0;
    }
}

