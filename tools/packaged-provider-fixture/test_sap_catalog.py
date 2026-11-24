"""Regression checks for the fixed SAP catalog fixture contract."""
import unittest
from urllib.parse import urlencode

from sap_catalog import page
from sap_peer import AZURE


class SapCatalogTest(unittest.TestCase):
    def test_azure_paginates_by_raw_count_with_cross_page_duplicate(self):
        query = {"scenarioId": "foundation-models", "executableIds": "azure-openai",
                 "status": "RUNNING", "$top": "50", "$skip": "0"}
        protocol, offset, first = page(AZURE, urlencode(query))
        self.assertEqual((protocol, offset, first["count"], len(first["resources"])), ("azure", 0, 6, 4))
        query["$skip"] = "4"
        _, offset, second = page(AZURE, urlencode(query))
        self.assertEqual((offset, len(second["resources"])), (4, 2))
        self.assertEqual(first["resources"][0], second["resources"][0])

    def test_rejects_unknown_page_or_unexpected_executable_filter(self):
        query = {"scenarioId": "orchestration", "status": "RUNNING", "$top": "50", "$skip": "0"}
        self.assertEqual(page(AZURE, urlencode(query))[0], "orchestration")
        self.assertIsNone(page(AZURE, urlencode(dict(query, executableIds="orchestration"))))
        self.assertIsNone(page(AZURE, urlencode(dict(query, **{"$skip": "1"}))))
        self.assertIsNone(page(AZURE, urlencode(query) + "&status=STOPPED"))
