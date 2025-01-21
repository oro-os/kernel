import gdb
import inspect
from functools import wraps
from typing import Callable, List, Optional
from . import SYMBOLS, QEMU
from ..log import warn, debug, error, log
from .backtrace import get_backtrace, log_backtrace, warn_backtrace, error_backtrace


def param(method: Callable[[object, bool], None]):
    """
    Decorator that marks the method as defining a GDB boolean parameter.
    The method's docstring is used for the set_doc/show_doc,
    and the method's name is the name of the parameter (e.g. "enabled", "verbose").
    """
    method._is_service_param = True
    return method


def hook(_func=None, *, symbol=None):
    """
    Decorator that marks a method as a 'service breakpoint' handler.
    If `symbol` is not provided, we'll default to the function name
    (with possible transformations in the attach logic).

    We'll also introspect the method's arguments so that each argument
    (besides 'self') is automatically parsed via gdb.parse_and_eval("<arg_name>").
    """

    def decorator(func):
        sig = inspect.signature(func)
        param_names = [p.name for p in sig.parameters.values() if p.name != "self"]

        func._is_service_breakpoint = True
        func._breakpoint_symbol = symbol or func.__name__
        func._breakpoint_param_names = param_names

        return func

    if _func is not None and callable(_func):
        return decorator(_func)

    return decorator


class OroService:
    """
    Base Oro GDB debug suite ("dbgutil") service, which allows
    custom logic to react to in-kernel events happening in near-realtime.
    """

    def __new__(cls, *args, **kwargs):
        service_name = getattr(cls, "_service_name")
        param_methods = getattr(cls, "_param_methods")
        instance = super(OroService, cls).__new__(cls)
        instance._service_name: Optional[str] = None  # set by @service
        instance._param_objects = {}
        instance._param_values = {}
        instance._breakpoints = {}
        for param_name, method in param_methods:
            full_name = f"{service_name}-{param_name}"
            doc = method.__doc__ or "No documentation provided."
            param_obj = ServiceParam(instance, param_name, full_name, doc)
            instance._param_objects[param_name] = param_obj
            instance._param_values[param_name] = param_obj.value
        return instance

    def __getitem__(self, param_name):
        """Allows self["name"] to get the parameter value."""
        return self._param_values[param_name]

    def __setitem__(self, param_name, new_value):
        """Allow self["name"] = True to set the parameter."""
        param_obj = self._param_objects.get(param_name)
        if param_obj is None:
            raise (
                ValueError(
                    f"oro service '{type(self)._service_name}' has no param '{param_name}'"
                )
            )
        else:
            param_obj.value = bool(new_value)
            param_obj.handle_value_changed(bool(new_value))

    def disable_breakpoint(self, sym):
        """Disables the given breakpoint by its symbol name."""
        bp = self._breakpoints.get(sym)
        if bp is not None:
            bp.enabled = False
            return True
        return False

    def enable_breakpoint(self, sym):
        """Enables the given breakpoint by its symbol name."""
        bp = self._breakpoints.get(sym)
        if bp is not None:
            bp.enabled = True
            return True
        return False

    def _log(self, *args, **kwargs):
        """Log a message with the service's name as the prefix."""
        log(f"{type(self)._service_log_tag}:", *args, **kwargs)

    def _warn(self, *args, **kwargs):
        """Log a warning with the service's name as the prefix."""
        warn(f"{type(self)._service_log_tag}:", *args, **kwargs)

    def _debug(self, *args, **kwargs):
        """Log a debug message with the service's name as the prefix."""
        debug(f"{type(self)._service_log_tag}:", *args, **kwargs)

    def _error(self, *args, **kwargs):
        """Log an error with the service's name as the prefix."""
        error(f"{type(self)._service_log_tag}:", *args, **kwargs)

    def _log_backtrace(self, bt=None):
        """Log a backtrace with the service's name as the prefix."""
        if bt is None:
            bt = get_backtrace()
        log_backtrace(type(self)._service_log_tag, bt)

    def _warn_backtrace(self, bt=None):
        """Log a warning backtrace with the service's name as the prefix."""
        if bt is None:
            bt = get_backtrace()
        warn_backtrace(type(self)._service_log_tag, bt)

    def _error_backtrace(self, bt=None):
        """Log an error backtrace with the service's name as the prefix."""
        if bt is None:
            bt = get_backtrace()
        error_backtrace(type(self)._service_log_tag, bt)

    def attach(self):
        """Attach all breakpoints if enabled. Called, e.g., when 'enabled' goes True."""
        self.detach()  # remove old first

        if not self._param_values.get("enabled", False):
            return

        found_symbols = []
        for _, method in inspect.getmembers(self, predicate=callable):
            if getattr(method, "_is_service_breakpoint", False):
                symbol_name = method._breakpoint_symbol
                sym = SYMBOLS.get_if_tracked(symbol_name)
                if sym is None:
                    self._warn(f"attach failed; missing symbol: {symbol_name}")
                    return
                found_symbols.append((symbol_name, sym, method))

        for sym, canon_sym, method in found_symbols:
            bp = _ServiceMethodBreakpoint(self, method, canon_sym)
            self._breakpoints[sym] = bp

        self._debug("attached")

    def detach(self):
        """Detach all breakpoints. Called, e.g., when 'enabled' goes False."""
        for bp in self._breakpoints.values():
            bp.delete()
        self._breakpoints.clear()

    def clear(self, reattach=True):
        """
        Called by QEMU.on_started(...) or similar. Overridable.
        """
        self.detach()
        if reattach and self._param_values.get("enabled", False):
            self.attach()


class ServiceParam(gdb.Parameter):
    def __new__(cls, service, param_name, fulle_param_name, doc, *args, **kwargs):
        newcls = type("ServiceParam", (ServiceParam, gdb.Parameter), {"__doc__": doc})
        return super().__new__(newcls)

    def __init__(
        self, service: OroService, param_name: str, full_param_name: str, doc: str
    ):
        super().__init__(full_param_name, gdb.COMMAND_DATA, gdb.PARAM_BOOLEAN)
        self._service = service
        self._param_name = param_name
        self.set_doc = doc
        self.show_doc = doc
        self.value = False  # default
        self.__doc__ = doc

    def get_set_string(self):
        new_val = bool(self.value)
        self.handle_value_changed(new_val)
        return ""

    def handle_value_changed(self, new_val: bool):
        self._service._param_values[self._param_name] = new_val
        method = getattr(self._service, self._param_name, None)
        if callable(method) and getattr(method, "_is_service_param", False):
            method(new_val)


def service(service_name: str, /, tag: str = None):
    """
    Class decorator that:
      1. Instantiates your service class immediately.
      2. Discovers `@param`-decorated methods => GDB parameters.
      3. Ensures there's always an 'enabled' param if none is defined.
      4. Binds the service name + param name => e.g. "oro-my-service-enabled".
    """

    if tag is None:
        tag = service_name

    def decorator(cls):
        param_methods = []
        for name, method in inspect.getmembers(cls, predicate=callable):
            if getattr(method, "_is_service_param", False):
                param_methods.append((name, method))

        has_enabled = any(name == "enabled" for name, _ in param_methods)
        if not has_enabled:

            def default_enabled(self, on):
                """Whether the service is enabled (default)."""
                if on:
                    self.attach()
                else:
                    self.detach()

            default_enabled._is_service_param = True
            setattr(cls, "enabled", default_enabled)
            param_methods.append(("enabled", default_enabled))

        setattr(cls, "_service_name", service_name)
        setattr(cls, "_param_methods", param_methods)
        setattr(cls, "_service_log_tag", tag)

        instance = cls()
        if not isinstance(instance, OroService):
            raise TypeError("@service-decorated class must derive from OroService")

        SYMBOLS.on_loaded(instance.attach)
        QEMU.on_started(instance.clear)

        cls._instance = instance

        return instance

    return decorator


class _ServiceMethodBreakpoint(gdb.Breakpoint):
    def __init__(self, service_obj: OroService, method: Callable, location):
        super().__init__(location, internal=True, qualified=True)
        self._service = service_obj
        self._method = method
        self._arg_names = getattr(method, "_breakpoint_param_names", [])

    def stop(self):
        arg_values = []
        for name in self._arg_names:
            try:
                val = gdb.parse_and_eval(name)
                arg_values.append(int(val))
            except gdb.error:
                self._service._warn(
                    f"could not parse hook expression '{name}' during breakpoint for symbol: {self.location}"
                )
                arg_values.append(None)

        self._method(*arg_values)
        return False  # continue execution
