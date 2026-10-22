namespace AnyCaptions.Api.Domain.Entities;

public sealed class UserLogin
{
    public Guid Id { get; set; } = Guid.NewGuid();
    public Guid UserId { get; set; }
    public required string Provider { get; set; }
    public required string ProviderKey { get; set; }
    public User User { get; set; } = null!;
}
