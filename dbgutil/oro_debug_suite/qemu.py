import socket
import struct
from .backend import Backend

PROMPT = b"(qemu) "
DEFAULT_ENDPOINT = "localhost:4444"


def parse_connection(connection):
    """
    Parses a connection string (e.g. "localhost:4444", "localhost" or ":4444")
    or tuple (e.g. ("localhost", 4444)) into a tuple (host, port) for use by the
    QEMU client connection.
    """

    if isinstance(connection, str):
        if ":" not in connection:
            connection += ":4444"
        [host, port] = connection.split(":")
        host = host.strip()
        port = port.strip()
        if not port.isdigit():
            raise ValueError(f"invalid port '{port}'")
        if not host:
            host = "localhost"
        return (host, int(port))
    elif isinstance(connection, tuple):
        if len(connection) != 2:
            raise ValueError("connection tuple must have 2 elements")
        [host, port] = connection
        if not isinstance(host, str):
            raise ValueError("host must be a string")
        host = host.strip()
        if isinstance(port, str):
            port = port.strip()
            if not port.isdigit():
                raise ValueError(f"invalid port '{port}'")
            port = int(port)
        if not isinstance(port, int):
            raise ValueError("port must be an integer")
        return (host, port)
    else:
        raise ValueError("connection must be a string or tuple")


class QemuConnection(object):
    """
    A low-level request/response client for QEMU monitor connections
    over TCP.
    """

    def __init__(self, endpoint=DEFAULT_ENDPOINT):
        """
        Connects to a QEMU monitor instance over TCP at the
        given connection string.
        """

        endpoint = endpoint or DEFAULT_ENDPOINT

        self._endpoint = parse_connection(endpoint)

        self._socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._socket.connect(self._endpoint)
        self._socket.setsockopt(
            socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 1)
        )

        self._read_response()  # pop off version + initial prompt

    @property
    def endpoint(self):
        """
        The connection string used to connect to the QEMU monitor.
        """

        (host, port) = self._endpoint

        return f"{host}:{port}"

    @property
    def is_connected(self):
        return self._socket is not None

    def close(self):
        """
        Closes the connection to the QEMU monitor.
        """

        if self.is_connected:
            self._socket.close()
            self._socket = None

    def request(self, data):
        """
        Sends a request to the QEMU monitor and returns the response.
        """

        if not self.is_connected:
            raise RuntimeError("not connected to QEMU monitor")

        if not isinstance(data, bytes):
            data = data.encode()
        self._socket.send(data)
        self._socket.send(b"\n")

        # Skip the first line of the response as it's an echo of the command
        # (and usually incomplete)
        while self._socket.recv(1) != b"\n":
            pass

        return self._read_response()

    def _read_response(self):
        """
        Reads a message from the QEMU monitor until the prompt is reached.
        """

        if not self.is_connected:
            raise RuntimeError("not connected to QEMU monitor")

        response = b""
        while True:
            end_idx = len(response)
            should_return = True

            for prompt_byte in PROMPT:
                b = self._socket.recv(1)
                response += b
                if b[0] != prompt_byte:
                    should_return = False
                    break

            if should_return:
                return response[:end_idx].decode("utf-8", "replace").strip()


class QemuBackend(Backend):
    def __init__(self, connection):
        super(QemuBackend, self).__init__()
        if not isinstance(connection, QemuConnection):
            raise ValueError("connection must be a QemuConnection instance")

        self.__connection = connection

    @property
    def connection(self):
        return self.__connection

    def read_physical(self, addr, length):
        if not isinstance(addr, int):
            raise ValueError("addr must be an integer")
        if not isinstance(length, int):
            raise ValueError("length must be an integer")

        if length > 4096:
            raise ValueError(
                "length must be <= 4KiB (cowardly refusing to read too much memory; this is probably a bug at the callsite)"
            )

        response = self.__connection.request(f"xp /{length}b {addr}")

        bytes = b""

        for line in response.split("\n"):
            line = line.strip()
            if not line:
                continue
            split = line.split(":")
            if len(split) != 2:
                raise ValueError(f"unexpected response line: {line}")
            [_, data] = split
            data = data.strip().split(" ")
            if len(data) > 8:
                raise ValueError(f"unexpected response line: {line}")

            for byte in data:
                if len(byte) != 4:
                    raise ValueError(f"unexpected byte format: {byte}")
                bytes += int(byte[2:], 16).to_bytes(
                    1, "little"
                )  # byte order doesn't matter here.

        assert len(bytes) == length, f"expected {length} bytes, got {len(bytes)}"

        return bytes
