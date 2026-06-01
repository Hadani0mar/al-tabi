SET NOCOUNT ON;
DECLARE @sf tinyint;
DECLARE @S_ID int = 0;
DECLARE @TRAN_NO int = 0;
DECLARE @TB varchar(10) = 'A';
DECLARE @dt datetime = GETDATE();

EXEC dbo.SALE_INVOICE_SAVE
  @S_ID = @S_ID,
  @S_DATE = @dt,
  @CUST_ID = 1,
  @CUST_NAME = 'test',
  @COMM_ID = 0,
  @S_STATUES = 0,
  @S_NOTE = '',
  @S_DISCOUNT = 0,
  @S_TAX1 = 0,
  @S_TAX2 = 0,
  @S_SHIPMENT = 0,
  @USERS_ID = 11,
  @COMM_PERCENT_FLAG = 0,
  @Flag = 5,
  @BRANCH = 0,
  @ACC_DEBIT = 0,
  @ACC_CREDIT = 0,
  @ACC_DIS_DEBIT = 0,
  @ACC_TAX_CREDIT = 0,
  @TRAN_ID = 0,
  @TRAN_NO = @TRAN_NO,
  @TRAN_BARNCH = @TB,
  @CB_TAKE = 0,
  @ACC_T_DEBIT1 = 0,
  @ACC_T_CREDIT1 = 0,
  @ACC_T_M_S_DEBIT_S1 = 0,
  @ACC_T_M_S_CREDIT_S1 = 0,
  @G_DEFULT_BANK1 = 0,
  @STATE_FLAG = @sf OUTPUT;

SELECT @sf AS sf;
