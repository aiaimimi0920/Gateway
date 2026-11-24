"""Synthetic upstream for real Command Code scheduler ticks; no forwarding."""
from scheduler_product import PROMPT, ScheduledChatPeer, SchedulerProduct

HOST = "api.commandcode.ai"
PATH = "/provider/v1/chat/completions"


class CommandCodePeer(ScheduledChatPeer):
    host = HOST
    key_prefix = "cpk-commandcode-fixture-"
    post_paths = (PATH,)


COMMANDCODE = SchedulerProduct("Command Code", "COMMANDCODE", "commandcode", "packaged-commandcode-",
                               "commandcode-api", "/provider/v1", CommandCodePeer)
