let
    Source = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
    Sheet  = Source{[Item="Sales",Kind="Sheet"]}[Data],
    Result = List.Select({null, "B"}, each _ <> null)
in
    Result
