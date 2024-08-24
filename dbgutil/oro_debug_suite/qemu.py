# Documentation on the supported QMP protocol here:
# https://gitlab.com/qemu-project/qemu/-/tree/master/qapi?ref_type=heads

import subprocess
import tempfile
import shutil
import gdb  # type: ignore
import threading
from qemu.qmp import QMPClient, Runstate  # type: ignore
import asyncio
import os
from os import path
from queue import SimpleQueue
import time
import signal
import errno
from .log import debug, error
import re

GVA2GPA_PATTERN = re.compile(r"gpa: 0x([0-9a-fA-F]+)\r\n")


class QmpThread(threading.Thread):
    def __init__(self, qmp_fifo_path):
        super().__init__()
        self.__qmp_path = qmp_fifo_path
        self.__qmp = None
        self.__loop = asyncio.new_event_loop()

    def __on_request(self, request, response_queue):
        async def _on_request_async(request, response_queue):
            try:
                response = await self.__qmp.execute_msg(request)
                response_queue.put_nowait((response, None))
            except Exception as e:
                response_queue.put_nowait((None, e))

        self.__loop.create_task(_on_request_async(request, response_queue))

    def request(self, request, **mapping):
        request_msg = QMPClient.make_execute_msg(request, mapping, oob=False)

        if self.__loop.is_closed():
            raise RuntimeError("QMP client has been closed")
        response_queue = SimpleQueue()
        self.__loop.call_soon_threadsafe(self.__on_request, request_msg, response_queue)

        (good, bad) = response_queue.get()
        if bad is not None:
            raise bad
        return good

    def run(self):
        self.__loop.run_until_complete(self.__run())
        self.__loop.close()

    async def __run(self):
        self.__qmp = QMPClient("oro-kernel")
        await self.__qmp.connect(self.__qmp_path)
        while True:
            # We've shut down and the connection is now IDLE.
            rs = await self.__qmp.runstate_changed()
            if rs == Runstate.IDLE:
                break

    async def __disconnect(self):
        if self.__qmp is not None and self.__qmp.runstate == Runstate.RUNNING:
            try:
                await self.__qmp.disconnect()
            except Exception as e:
                pass

    def __shutdown(self):
        self.__loop.create_task(self.__disconnect())

    def shutdown(self):
        try:
            self.__loop.call_soon_threadsafe(self.__shutdown)
        except:
            # Loop's already been closed.
            pass
        self.join()


def wait_for_file(file_path, timeout=5):
    """
    Waits for a file to exist, with a timeout.
    """

    for _ in range(timeout * 10):
        if path.exists(file_path):
            return
        time.sleep(0.1)

    raise TimeoutError(f"file not found (timed out waiting for it): {file_path}")


class QemuProcess(object):
    """
    Spawns QEMU with the given arguments and provides a way to connect GDB to it.

    Note that `-qmp` and `-gdb` arguments are automatically added to the arguments
    and should not be specified by the caller.
    """

    def __init__(self, args, **kwargs):
        self.__tmpdir = tempfile.mkdtemp()
        self.__qmp_path = path.join(self.__tmpdir, "qmp.sock")
        self.__qmp = QmpThread(self.__qmp_path)
        self.__gdbsrv_path = path.join(self.__tmpdir, "gdbsrv.sock")

        self.__readphys_fifo_path = path.join(self.__tmpdir, "readphys.fifo")
        os.mkfifo(self.__readphys_fifo_path)
        self.__readphys_fifo = os.open(
            self.__readphys_fifo_path,
            os.O_RDONLY | os.O_NONBLOCK | getattr(os, "O_BINARY", 0),
        )

        args = [
            *args,
            "-qmp",
            f"unix:{self.__qmp_path},server",
            "-gdb",
            f"unix:{self.__gdbsrv_path},server",
            "-S",
        ]

        debug("spawning QEMU with args:", repr(args))

        self.__process = subprocess.Popen(
            args,
            **kwargs,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            preexec_fn=lambda: signal.pthread_sigmask(
                signal.SIG_BLOCK, [signal.SIGINT]
            ),
        )

        wait_for_file(self.__qmp_path)
        self.__qmp.start()

    def poll(self):
        """
        Polls the underlying child process to check if it has terminated.
        """

        return self.__process.poll()

    def shutdown(self):
        """
        Safely shuts down the QEMU process and the QMP thread.
        """

        conn = gdb.selected_inferior().connection
        if isinstance(conn, gdb.RemoteTargetConnection) and conn.is_valid():
            details = conn.details
            if details == self.__gdbsrv_path:
                gdb.execute("disconnect", to_string=False, from_tty=False)

        if self.__qmp is not None:
            self.__qmp.shutdown()
            self.__qmp = None
        if self.__process is not None:
            if self.__process.poll() is None:
                self.__process.kill()
                self.__process.wait()
            self.__process = None
        if self.__readphys_fifo is not None:
            os.close(self.__readphys_fifo)
            self.__readphys_fifo = None
        shutil.rmtree(self.__tmpdir, ignore_errors=True)

    def __del__(self):
        self.shutdown()

    @property
    def pid(self):
        """
        The PID of the QEMU process, or None if the process has not been spawned /
        has already been terminated.
        """

        if self.__process is None:
            return None
        return self.__process.pid

    def connect_gdb(self):
        """
        Connects GDB to the QEMU process that was spawned.
        """

        wait_for_file(self.__gdbsrv_path)
        gdb.execute(
            f"target remote {self.__gdbsrv_path}", to_string=False, from_tty=False
        )

    def read_physical(self, addr, size):
        """
        Reads physical memory from the QEMU process.
        """

        if size <= 0:
            raise ValueError("size must be greater than 0")
        if addr < 0:
            raise ValueError("address must be non-negative")

        self.__qmp.request(
            "pmemsave", val=addr, size=size, filename=self.__readphys_fifo_path
        )

        result = b""

        while len(result) < size:
            try:
                result += os.read(self.__readphys_fifo, size - len(result))
            except OSError as err:
                if err.errno == errno.EAGAIN or err.errno == errno.EWOULDBLOCK:
                    pass
                else:
                    raise err

        assert len(result) == size
        return result

    def gva2gpa(self, addr):
        """
        Converts a guest virtual address to a guest physical address.

        Uses the QEMU monitor command `gva-to-gpa`.
        """

        result = self.monitor_command(f"gva2gpa {addr}")
        match = GVA2GPA_PATTERN.search(result)
        if match is None:
            error(f"failed to convert GVA to GPA: {result}")
            return None
        return int(match.group(1), 16)

    def monitor_command(self, command):
        """
        Sends a "human" command to the QEMU monitor.

        This is like sending a command via the text-based (non-QMP)
        QEMU monitor REPL.
        """

        return self.__qmp.request("human-monitor-command", **{"command-line": command})
