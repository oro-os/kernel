from ..log import warn
from ..qemu import QemuConnection, QemuBackend


class QemuService(object):
    """
    Manages a singleton connection to a QEMU monitor.
    """

    def __init__(self):
        self._connection = None

    @property
    def connection(self):
        """
        Returns the active QEMU monitor connection.
        Raises an exception if not connected.
        """

        if not self._connection:
            raise Exception("QEMU is not connected; use 'oro qemu connect' to connect")

        return self._connection

    @property
    def backend(self):
        """
        Returns a `Backend` implementation for the active QEMU connection.
        """

        return QemuBackend(self.connection)

    @property
    def is_connected(self):
        """
        Is the QEMU monitor connected?
        """

        return self._connection is not None

    def connect(self, connection=None):
        """
        Connects to a QEMU monitor. Kills an old connection if one exists.
        """

        if self.is_connected:
            warn("qemu: already connected; killing old connection")
            self._connection.close()

        self._connection = QemuConnection(connection)


QEMU = QemuService()
