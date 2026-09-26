import re


class GatewayRepositoryTextFixture:
    @staticmethod
    def _powershell_function_block(script: str, function_name: str) -> str:
        start = script.index(f"function {function_name}")
        end = script.find("\nfunction ", start + 1)
        if end < 0:
            end = len(script)
        return script[start:end]

    @staticmethod
    def _workflow_job_block(workflow: str, job_name: str) -> str:
        match = re.search(
            rf"(?ms)^  {re.escape(job_name)}:\r?$.*?(?=^  [A-Za-z0-9_-]+:\r?$|\Z)",
            workflow,
        )
        if match is None:
            raise AssertionError(f"missing workflow job: {job_name}")
        return match.group(0)

    @staticmethod
    def _assert_markers_in_order(text: str, *markers: str) -> None:
        positions = []
        for marker in markers:
            position = text.find(marker)
            if position < 0:
                raise AssertionError(f"missing ordered marker: {marker}")
            positions.append(position)
        if positions != sorted(positions):
            raise AssertionError(
                f"markers are out of order: {list(zip(markers, positions))}"
            )
