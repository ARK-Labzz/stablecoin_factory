use super::*;

#[account]
#[derive(InitSpace, Debug)]
pub struct FeeOperator {
    pub operator: Pubkey,
    pub bump: u8,
    pub _padding: [u8; 128],
}

const_assert_eq!(FeeOperator::INIT_SPACE, 161); 

impl FeeOperator {
    pub fn initialize(&mut self, operator: Pubkey) -> Result<()> {
        self.operator = operator;
        self.bump = 0; 
        Ok(())
    }
}